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
use assets::object::{NObjectType, SObjectType, Weapon};
use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::fixed::{ftoi, itof, Fixed};
use sim_core::rng::Rand;
use sim_core::tables::precompute_cossin;
use sim_core::vec::Vec2;

use crate::blit::draw_dirt_effect;
use crate::control::{
    process_aiming, process_movement, process_tasks, process_weapon_change, process_weapons,
    ControlConsts,
};
use crate::nobject::{nobject_create1, nobject_create2, nobject_process, NObjectOutcome};
use crate::physics::{worm_process_physics, worm_reactions, PhysicsConsts};
use crate::pool::{BloodPool, Pool};
use crate::sobject::{sobject_process, SObjectOutcome};
use crate::weapon::{blow_up, wobject_process, worm_fire, WObjectOutcome};

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

    /// Sets control bit `n`, mirroring C++ `Worm::Press` (`worm.hpp:199`:
    /// `control_states.Set(control, true)`).
    pub fn press(&mut self, n: u32) {
        self.set(n, true);
    }

    /// Clears control bit `n`, mirroring C++ `Worm::Release` (`worm.hpp:197`:
    /// `control_states.Set(control, false)`).
    pub fn release(&mut self, n: u32) {
        self.set(n, false);
    }

    /// Returns whether control bit `n` is set and **clears it**, mirroring C++
    /// `Worm::PressedOnce` (`worm.hpp:191-195`): read the bit, `Set(control,
    /// false)`, return the prior value. The ported control paths use this to
    /// consume an edge; because the driver re-`Unpack`s `control_states` from the
    /// scripted input each tick, the C++ `prev_control_states` edge detection
    /// degenerates to this per-tick read-and-clear (design doc,
    /// *Control-state mutation*).
    pub fn pressed_once(&mut self, n: u32) -> bool {
        let was = self.get(n);
        self.set(n, false);
        was
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

impl WormWeapon {
    /// Port of `WormWeapon::Available()` (`worm.hpp:35`): `loading_left == 0`.
    /// This is the Fire gate's reload predicate **only** — it deliberately
    /// ignores `ammo` and `delay_left`. The C++ Fire gate
    /// (`worm.cpp`, design doc *Fire gate*) tests `delay_left <= 0` (and ammo)
    /// **separately**; folding them in here would change the gate semantics.
    pub fn available(&self) -> bool {
        self.loading_left == 0
    }
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
    /// `Worm::last_killed_by_idx` (`worm.hpp:251`, default `-1`): the worm that
    /// last killed this one. Set by [`do_damage_direct`](WormState::do_damage_direct)
    /// when `health` falls to `<= 0`; feeds death attribution only (`worm.cpp:397`).
    /// **Not hashed** — adding it leaves every slice golden unchanged.
    pub last_killed_by_idx: i32,

    // --- Slice 3 control state (NOT hashed) -------------------------------
    // The non-hashed worm state the ported control/aiming paths read/write
    // across ticks to make the *hashed* fields (aiming_angle, control_states,
    // vel, weapon delays, ninjarope) evolve correctly. Defaults are the C++
    // ctor / `ResetWorms` constants (design doc, *Datamodel additions*; see
    // `from_init`). The control logic that reads these lands in later tasks.
    /// `Worm::aiming_speed` (`worm.hpp:226`): aim-angle velocity. Drives
    /// `aiming_angle`.
    pub aiming_speed: Fixed,
    /// `Worm::direction` (`worm.hpp:262`): which way the worm faces (0 left,
    /// 1 right). Clamps + flips `aiming_angle`.
    pub direction: i32,
    /// `Worm::movable` (`worm.hpp`, ctor sets `true`): gates aiming/movement.
    pub movable: bool,
    /// `Worm::able_to_jump` (`worm.hpp:228`): previous Jump-key edge state.
    pub able_to_jump: bool,
    /// `Worm::able_to_dig` (`worm.hpp:228`): previous dig edge state (dig body
    /// deferred; flag toggles only).
    pub able_to_dig: bool,
    /// `Worm::key_change_pressed` (`worm.hpp:229`): edge latch for the Change
    /// key in `ProcessWeaponChange`; gates the Left/Right `Release`.
    pub key_change_pressed: bool,
    /// `Worm::current_weapon` (`worm.hpp:250`; `ResetWorms` sets 0): selected
    /// weapon slot.
    pub current_weapon: i32,
    /// `Worm::fire_cone` (`worm.hpp:252`): firecone countdown (inert this slice;
    /// `ProcessWeapons` decrement, not hashed).
    pub fire_cone: i32,
    /// `Worm::leave_shell_timer` (`worm.hpp:253`): shell-drop countdown (inert;
    /// only `Worm::Fire` sets it, and gates a `rand()` branch — stays 0).
    pub leave_shell_timer: i32,

    // --- Slice 5d T1 dead/respawn runtime fields (NOT hashed) --------------
    // Written by the dead-worm `else` arm (`worm.cpp:431-450`) +
    // `BeginRespawn`/`DoRespawning`. Cross-checked against `stateHash.hpp`:
    // NONE of these appears in `HashGameState` (master) or `HashGameComponents`
    // (component), so adding them leaves every slice 1-5c golden byte-identical.
    // Defaults match a freshly-reset C++ worm (ctor + `ResetWorms`, game.cpp:155;
    // `ResetWorms` does NOT touch these four, so they keep their ctor/in-class
    // values).
    /// `Worm::logic_respawn` (`worm.hpp:223`, an `IVec2`; here [`Vec2`] is the
    /// `IVec2` port): the pixel-space drop-in cursor `BeginRespawn` seeds and
    /// `DoRespawning` walks toward `Ftoi(pos) - 80`. Default `(0,0)`
    /// (default-constructed `IVec2`). **Not hashed.**
    pub logic_respawn: Vec2,
    /// `Worm::ready` (`worm.hpp:234` `ready{false}`, but the ctor's initializer
    /// list sets `ready(true)` — `worm.hpp:179` — and `ResetWorms` does NOT
    /// reset it, so a freshly-reset worm is `ready == true`). Set true by the
    /// dead arm's `PressedOnce(kFire)`; the `DoRespawning` completion gate reads
    /// it then clears it. **Not hashed.**
    pub ready: bool,
    /// `Worm::make_sight_green` (`worm.hpp:236` `make_sight_green{false}`):
    /// cleared by the death block; render-only sight tint. Default false.
    /// **Not hashed.**
    pub make_sight_green: bool,
    /// `Worm::steerable_count` (`worm.hpp:267` `steerable_count{0}`): the
    /// steerable-object accumulator the dead arm zeroes each tick. Default 0.
    /// **Not hashed.**
    pub steerable_count: i32,
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
            last_killed_by_idx: -1, // worm.hpp:251 default

            // Post-`ResetWorms`/ctor control defaults (design doc, *Datamodel
            // additions*; verified against worm.hpp + game.cpp ResetWorms).
            aiming_speed: 0,
            direction: 0,
            movable: true, // ctor sets `movable(true)` (worm.hpp)
            able_to_jump: false,
            able_to_dig: false,
            key_change_pressed: false,
            current_weapon: 0, // ResetWorms sets `current_weapon = 0` (game.cpp:164)
            fire_cone: 0,
            leave_shell_timer: 0,

            // Slice 5d T1 dead/respawn runtime defaults (freshly-reset C++ worm;
            // `ResetWorms` does not touch these, so they keep their ctor/in-class
            // values). None hashed.
            logic_respawn: Vec2::zero(),
            ready: true, // ctor `ready(true)` (worm.hpp:179); ResetWorms keeps it
            make_sight_green: false,
            steerable_count: 0,
        }
    }

    /// Port of `Game::DoDamageDirect` (`game.cpp:546-553`). RNG-free. Subtracts
    /// `amount` (only when positive) from `health`; if that drops `health` to
    /// `<= 0`, records `by_idx` as the killer in [`last_killed_by_idx`]
    /// (`WormState::last_killed_by_idx`, not hashed — death attribution only).
    pub fn do_damage_direct(&mut self, amount: i32, by_idx: i32) {
        if amount > 0 {
            self.health -= amount;
            if self.health <= 0 {
                self.last_killed_by_idx = by_idx;
            }
        }
    }

    /// Port of `Game::DoDamage` (`game.cpp:567-589`) — the **normal-mode**
    /// (`kGmKillEmAll`) path. In every mode the function first runs
    /// [`do_damage_direct`](WormState::do_damage_direct); the additional
    /// `kGmScalesOfJustice` redistribution branch (`:570-587`, which heals the
    /// other worms via `DoHealingDirect`) is **DEFERRED** — Slice 5b is normal
    /// mode, so it is unreached. Bringing in game-mode plumbing 5b never needs
    /// would be premature; when a SoJ slice lands it ports the redistribution
    /// here. RNG-free.
    pub fn do_damage(&mut self, amount: i32, by_idx: i32) {
        self.do_damage_direct(amount, by_idx);
        // kGmScalesOfJustice redistribution (game.cpp:570-587) DEFERRED — normal
        // mode does nothing further. (No `DoHealingDirect` port: it is reachable
        // only through that branch.)
    }
}

// ---------------------------------------------------------------------------
// Object-pool element types (empty this slice; fields are what the hash reads)
// ---------------------------------------------------------------------------

/// A weapon bonus crate. Hash reads `x, y, timer, weapon, frame`.
///
/// `vel_y` mirrors C++ `Bonus::vel_y` (`bonus.hpp`): the fixed-point fall
/// velocity `Game::CreateBonus` writes (`game.cpp:250`, set to 0 at spawn) and
/// `Bonus::Process` integrates. It is **not** hashed (the C++ `stateHash` / Rust
/// `hash.rs` bonus fold reads `x, y, timer, weapon, frame` only), so adding it
/// leaves the bonus hash and slices 1-5b goldens unchanged. `Bonus::Process` (the
/// reader) is deferred to Slice-5c Task 3; carried now so the T2 spawn writes the
/// full C++ field set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bonus {
    pub x: i32,
    pub y: i32,
    pub timer: i32,
    pub weapon: i32,
    pub frame: i32,
    /// C++ `Bonus::vel_y`. Default 0. Not hashed (fall velocity for T3).
    pub vel_y: i32,
}

/// A weapon projectile. Hash reads `pos, vel, cur_frame, time_left, ty.id`.
///
/// `owner_idx` mirrors C++ `WObject::owner_idx` (`weapon.hpp:341`): the firing
/// worm's index, used for self-exclusion in the collide loop. It is **not**
/// hashed (the C++ `stateHash`/Rust `hash.rs` wobject fold omits it), so adding
/// it leaves the wobject hash and slices 1-3 goldens unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WObject {
    pub pos: Vec2,
    pub vel: Vec2,
    pub cur_frame: i32,
    pub time_left: i32,
    pub ty: Option<WeaponId>,
    /// Firing worm's index (C++ `owner_idx`). Default 0. Not hashed.
    pub owner_idx: i32,
}

/// A "sound/explosion" object. Hash reads `id, cur_frame`.
///
/// `x`/`y` are the object's pixel position and `anim_delay` its per-frame
/// animation countdown; all three mirror C++ `SObject` (`sobject.hpp:89-95`:
/// `int x, y; int id; int cur_frame; int anim_delay;`). They are read by
/// `SObject::Process` but are **not** hashed (the C++ `stateHash`/Rust `hash.rs`
/// sobject fold reads `id`+`cur_frame` only), so adding them leaves the sobject
/// hash and slices 1-4b goldens unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SObject {
    pub id: i32,
    /// Pixel X (C++ `x`). Default 0. Not hashed.
    pub x: i32,
    /// Pixel Y (C++ `y`). Default 0. Not hashed.
    pub y: i32,
    pub cur_frame: i32,
    /// Per-frame animation countdown (C++ `anim_delay`). Default 0. Not hashed.
    pub anim_delay: i32,
}

/// A non-weapon object (debris/splinters). Hash reads `pos, vel, cur_frame, ty.id`.
///
/// `owner_idx` mirrors C++ `NObject::owner_idx` (`nobject.hpp:208`): the firing
/// worm's index, used for self-exclusion / damage attribution. `time_left` mirrors
/// C++ `NObject::time_left` (`nobject.hpp:205`): the explode countdown. Neither is
/// hashed (the C++ `stateHash`/Rust `hash.rs` nobject fold omits them), so adding
/// them leaves the nobject hash and slices 1-4b goldens unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NObject {
    pub pos: Vec2,
    pub vel: Vec2,
    pub cur_frame: i32,
    pub ty: Option<i32>,
    /// Firing worm's index (C++ `owner_idx`). Default 0. Not hashed.
    pub owner_idx: i32,
    /// Explode countdown (C++ `time_left`). Default 0. Not hashed.
    pub time_left: i32,
}

/// A blood particle (C++ `BObject`, `bobject.hpp`). Hash reads `pos` only.
///
/// `vel` mirrors C++ `BObject::vel`: read by [`crate::bobject::bobject_process`]
/// (`pos += vel`, and gravity adds to `vel.y` in background air). `color` mirrors
/// C++ `BObject::color = rand(NumBloodColours) + FirstBloodColour`: the spawn DRAW
/// is load-bearing (advances the shared RNG), but the value is never read by
/// Process and is render-only. **Neither is hashed** (the C++ `stateHash` /
/// `hash.rs` BObject fold reads `pos.x`/`pos.y` only), so adding them leaves the
/// bobject hash and slices 1-5a goldens unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BObject {
    pub pos: Vec2,
    /// C++ `BObject::vel`. Default `(0,0)`. Not hashed.
    pub vel: Vec2,
    /// C++ `BObject::color`. Default 0. Not hashed (render-only).
    pub color: i32,
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

/// `Material::kDirt` (`material.hpp:7`): destructible dirt.
pub const MAT_DIRT: u8 = 1 << 0;
/// `Material::kDirt2` (`material.hpp:8`): second dirt variant.
pub const MAT_DIRT2: u8 = 1 << 1;
/// `Material::kRock` (`material.hpp:9`): solid rock.
pub const MAT_ROCK: u8 = 1 << 2;
/// `Material::kBackground` (`material.hpp:10`): the flag bit a "background"
/// (empty/walkable) material carries.
pub const MAT_BACKGROUND: u8 = 1 << 3;
/// `Material::DirtRock()` (`material.hpp:22`): `flags & (kDirt|kDirt2|kRock)`,
/// i.e. bits 0|1|2 — the "solid to projectiles" combination. `kBackground`
/// (bit 3) is deliberately excluded.
pub const MAT_DIRT_ROCK: u8 = MAT_DIRT | MAT_DIRT2 | MAT_ROCK;

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

    /// Port of `Level::Inside(x, y)` (`level.hpp:132-135`): a **true** range
    /// check `0 <= x < width && 0 <= y < height`.
    ///
    /// The C++ writes it as `(unsigned)x < (unsigned)width && (unsigned)y <
    /// (unsigned)height`; for non-negative `width`/`height` that unsigned trick
    /// is exactly the signed range check (a negative coordinate reinterprets to a
    /// huge unsigned and fails). This is **distinct** from
    /// [`checked_mat_background`](Self::checked_mat_background), which flattens
    /// `x + y*width` with **no separate x-bounds check** and can wrap a negative
    /// `x` into a wrong-row in-range pixel. `WObject::Process` tests `Inside`
    /// *separately* before the material probe (`weapon.cpp:249`).
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    /// Port of `Level::PixelMat(x, y).DirtRock()` gated by `Inside`. Returns
    /// `false` when `!inside` (the C++ collision tests `!Inside(...)` first, so a
    /// projectile leaving the level never reads a wrapped pixel). When inside,
    /// looks up `material_flags[material_id[idx]]` (the same flattened index as
    /// [`checked_mat_background`](Self::checked_mat_background)) and tests the
    /// `DirtRock` bit set (`material.hpp:22`, bits 0|1|2).
    pub fn dirt_rock(&self, x: i32, y: i32) -> bool {
        if !self.inside(x, y) {
            return false;
        }
        // inside() guarantees 0 <= x < width and 0 <= y < height, so the
        // flattened index is in range; no wrap concern here.
        let idx = (x + y * self.width) as usize;
        let flags = self.material_flags[self.material_id[idx] as usize];
        (flags & MAT_DIRT_ROCK) != 0
    }

    // ----- Slice 4b flag-read predicates --------------------------------------
    //
    // SHAPE CHOICE: these take **in-bounds `(x, y)`** (NOT a flat index). They
    // index the flattened `x + y*width` directly with **no `inside` gate** —
    // the same inner read `dirt_rock` performs *after* its gate. The caller in
    // Task 1 (`DrawDirtEffect`) clips its 16x16 stamp to the level *before*
    // probing, so every coordinate it passes is already in range; pushing a
    // redundant per-pixel bounds check into these helpers would be wasted work
    // (and would diverge from the C++ `PixelMat(x, y).Background()` shape, which
    // is also unchecked once the caller has clipped). `(x, y)` matches the 4a
    // probes (`checked_mat_background`/`dirt_rock`) so Task 1 walks one
    // coordinate convention throughout. The companion writer `set_material`
    // takes a flat index because Task 1 computes the index once per pixel and
    // both reads and the write share it.

    /// Background bit (`material.hpp:18`) of the **in-bounds** pixel `(x, y)`.
    /// Looks up `material_flags[material_id[x + y*width]]` and tests
    /// [`MAT_BACKGROUND`]. In-bounds only (see the shape note above).
    pub fn background(&self, x: i32, y: i32) -> bool {
        let idx = (x + y * self.width) as usize;
        (self.material_flags[self.material_id[idx] as usize] & MAT_BACKGROUND) != 0
    }

    /// Either dirt bit (`material.hpp`: `kDirt | kDirt2`) of the in-bounds
    /// pixel `(x, y)` — the "any destructible dirt" predicate `DrawDirtEffect`
    /// uses to decide a pixel may be dug.
    pub fn any_dirt(&self, x: i32, y: i32) -> bool {
        let idx = (x + y * self.width) as usize;
        (self.material_flags[self.material_id[idx] as usize] & (MAT_DIRT | MAT_DIRT2)) != 0
    }

    /// First dirt bit ([`MAT_DIRT`]) of the in-bounds pixel `(x, y)`.
    pub fn dirt(&self, x: i32, y: i32) -> bool {
        let idx = (x + y * self.width) as usize;
        (self.material_flags[self.material_id[idx] as usize] & MAT_DIRT) != 0
    }

    /// Second dirt bit ([`MAT_DIRT2`]) of the in-bounds pixel `(x, y)`.
    pub fn dirt2(&self, x: i32, y: i32) -> bool {
        let idx = (x + y * self.width) as usize;
        (self.material_flags[self.material_id[idx] as usize] & MAT_DIRT2) != 0
    }

    /// The FIRST `material_id` writer (used by `DrawDirtEffect` in Task 1 to
    /// stamp destroyed terrain). Sets `material_id[idx] = v` and **nothing
    /// else**: the C++ engine also maintains a derived `materials` flag cache, a
    /// `display_valid` flag and a dirty-rect list, but those are render/derived
    /// state that the Rust port omits (they are not hashed — the level hash
    /// reads `material_id` only), so a single store is the whole operation here.
    /// A subsequent flag-read ([`background`](Self::background)/[`dirt`](Self::dirt)
    /// /…) reflects the new material via `material_flags`.
    pub fn set_material(&mut self, idx: usize, v: u8) {
        self.material_id[idx] = v;
    }

    /// Port of `Level::Pixel(x, y)` (`level.hpp:66`): the raw palette index
    /// `material_id[x + y*width]` of the **in-bounds** pixel `(x, y)`. In this port
    /// `material_id` *is* the C++ pixel buffer (per-pixel palette colour), so this
    /// returns the colour value `BObject::Process` bands on (`1..=2`, `77..=79`).
    /// In-bounds only (the caller gates `inside`, matching `bobject.cpp:24-27`).
    pub fn pixel(&self, x: i32, y: i32) -> i32 {
        self.material_id[(x + y * self.width) as usize] as i32
    }

    /// Rock bit ([`MAT_ROCK`], `material.hpp:17`) of the in-bounds pixel `(x, y)`.
    /// The `BObject::Process` rock-landing probe (`bobject.cpp:43`). In-bounds only.
    pub fn rock(&self, x: i32, y: i32) -> bool {
        let idx = (x + y * self.width) as usize;
        (self.material_flags[self.material_id[idx] as usize] & MAT_ROCK) != 0
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
    /// The TC constants/hacks (aim/move/jump/ninjarope) the worm control +
    /// aiming paths read. Built once from the same TC; not hashed.
    pub control: ControlConsts,
    /// The resolved weapon parameter table (C++ `common.weapons`). `Fire` /
    /// `WObject::Process` read the firing weapon's params from here. Empty in
    /// slices 1-3 (no firing); not hashed.
    pub weapons: Vec<Weapon>,
    /// The precomputed 128-step cos/sin direction table (C++ `cossin_table`),
    /// from [`sim_core::tables::precompute_cossin`]. `Fire` reads it for the
    /// muzzle velocity / firing position and recoil. Not hashed.
    pub cossin: [Vec2; 128],
    /// The TC `[hacks].SignedRecoil` flag (C++ `common.h[HSignedRecoil]`). Read
    /// only by [`worm_fire`]'s recoil step; a `recoil >= 128` is reinterpreted as
    /// `recoil - 256` when set. Built from the TC, **not** hashed (slices 1-3
    /// never fire, so the value is inert for them).
    pub h_signed_recoil: bool,
    /// The 16x16 large-sprite bank (C++ `common.largeSprites`). Slice-4b Task 1's
    /// `DrawDirtEffect` reads a texture's `mframe` sprite from here (`sprite(n)`
    /// is a 256-byte 16x16 slice) to decide which crater pixels become
    /// background. Loaded from `sprites/large.tga`; **not** hashed (the level
    /// hash reads the material map). Slices 1-4a never index it (no dig runs), so
    /// they pass an empty bank and stay byte-identical.
    pub large_sprites: SpriteSet,
    /// The 7x7 small-sprite bank (C++ `common.small_sprites`,
    /// `small_sprites.Allocate(7, 7, 130)`). `NObject::Process`'s ground-explode
    /// arm reads `small_sprites.SpritePtr(start_frame + cur_frame)` to
    /// `BlitImageOnMap` an object's image into the level (the spent SHELL paints a
    /// 7x7 stamp onto `material_id` when it lands; Slice-4d). **Not** hashed (the
    /// level hash reads the painted `material_id`). Defaults to an empty bank;
    /// slices that never land a `draw_on_map` nobject never index it, so they stay
    /// byte-identical. The differential harness sets this field after `new`
    /// (loaded from `sprites/small.tga`) — kept out of the `new` arg list so the
    /// existing slice call sites are unchanged.
    pub small_sprites: SpriteSet,
    /// The TC texture table (C++ `common.textures`, `TcConfig.textures`).
    /// `DrawDirtEffect` looks up `textures[dirt_effect]` for its `mframe`/`sframe`
    /// frames + `ndrawback`. **Not** hashed; slices 1-4a never index it (the fan's
    /// `dirt_effect = -1`, so `draw_dirt_effect` never runs), so they pass an
    /// empty Vec and stay byte-identical.
    pub textures: Vec<Texture>,
    /// The TC sound/explosion-object parameter table (C++ `common.sobjects`,
    /// indexed by `SObject::id`). Slice-4c's `SObject::Process` reads a live
    /// object's `sobject_types[id]` for its `anim_delay`/`num_frames`/damage/
    /// dirt-effect params. **Not** hashed; slices 1-4b never spawn an sobject, so
    /// they pass an empty `Vec` and stay byte-identical. Carried now so Slice-4c's
    /// behaviour tasks have the table in place.
    pub sobject_types: Vec<SObjectType>,
    /// The TC non-weapon-object parameter table (C++ `common.nobjects`, indexed
    /// by `NObject::ty`). Slice-4c's `NObject::Process` reads a live object's
    /// `nobject_types[ty]` for its speed/gravity/bounce/expl-ground params.
    /// **Not** hashed; slices 1-4b never spawn an nobject, so they pass an empty
    /// `Vec` and stay byte-identical. Carried now so Slice-4c's behaviour tasks
    /// have the table in place.
    pub nobject_types: Vec<NObjectType>,
    /// C++ `Settings::loading_time` (`settings.hpp:79`): the reload countdown
    /// scaled into `loading_left` when a weapon depletes (Slice-4d reload). The
    /// in-game default is 100, but the **oracle dumper sets it to 0**
    /// (`sim_physics_dump.cpp`), so the slice-4d golden uses 0 (instant reload);
    /// pass the value the scenario's dumper used. **Not** hashed (settings scalar).
    pub settings_loading_time: i32,
    /// C++ `Settings::load_change` (`settings.hpp:75`): whether a weapon can be
    /// changed while still loading (Slice-4d weapon-change gate). Default `true`.
    /// **Not** hashed (settings scalar).
    pub load_change: bool,
    /// C++ `Settings::blood` (`settings.hpp:70`): the blood-amount scaler. The
    /// sobject worm-damage arm spawns `blood * power_sum / 100` blood nobjects
    /// (`sobject.cpp:96`). The dumper default is 100 (never overridden), so the
    /// goldens use 100. **Not** hashed (settings scalar); slices 1-5a never enter
    /// the damage arm (worms out of range), so the value is inert for them.
    pub blood: i32,
    /// C++ `common.c[CNumBloodColours]` (`constants`, `bobject.cpp:12`): the count
    /// the blood-trail's `CreateBObject` rolls `rand(NumBloodColours)` against. The
    /// DRAW advances the shared RNG (load-bearing) even though the resulting colour
    /// is never hashed. **Not** hashed (TC scalar). Defaulted to 0 here and assigned
    /// the real TC value by the differential harness AFTER `new` (mirrors
    /// [`small_sprites`](Self::small_sprites)); slices 1-5a never spawn a bobject
    /// (blood-trail dormant), so the value is inert and the goldens stay identical.
    pub num_blood_colours: i32,
    /// C++ `common.c[CFirstBloodColour]` (`bobject.cpp:12`): the base added to the
    /// blood-colour roll. Render-only (the colour is never hashed). Defaulted to 0;
    /// harness-assigned after `new`. Inert for slices 1-5a.
    pub first_blood_colour: i32,
    /// C++ `common.c[CBObjGravity]` (`bobject.cpp:31`): the per-tick `vel.y` add a
    /// blood particle gets in background air. Load-bearing for the bobject `pos`
    /// hash over time (gravity -> vel -> future pos). Defaulted to 0; harness-assigned
    /// after `new`. Inert for slices 1-5a (no bobjects).
    pub bobj_gravity: i32,
    /// C++ `Settings::max_bonuses` (`settings.hpp:69`, in-game default 4): the cap the
    /// per-tick **bonus-drop roll** gates on (`game.cpp:359`). The roll `if (max_bonuses
    /// > 0 && rand(CBonusDropChance) == 0) CreateBonus()` fires in [`process_frame`]
    /// only when this is `> 0`; the `&&` short-circuits so `max_bonuses == 0` draws NO
    /// rand. **Defaulted to 0** here (NOT in the `new` arg list — set post-`new` by the
    /// difftest, like the blood consts) so every slice 1-5b scenario keeps it 0 and the
    /// roll never fires => those goldens stay byte-identical (no regen). T6's 5c
    /// scenario threads a `max_bonuses > 0` to make the pool go live. **Not** hashed.
    pub settings_max_bonuses: i32,
    /// C++ `common.c[CBonusDropChance]` (`constants.hpp`): the bound the per-tick
    /// bonus-drop roll draws `rand(CBonusDropChance)` against (`game.cpp:360`). Read
    /// only when `settings_max_bonuses > 0` (the `&&` short-circuits before it
    /// otherwise), so it is inert while `max_bonuses == 0` (slices 1-5b). Defaulted to
    /// 0 here; the difftest assigns the real TC value (`tc.constants.BonusDropChance`)
    /// after `new`, the same post-`new` pattern as the blood consts. **Not** hashed
    /// (TC scalar; the resulting bonus position would hash, but spawning is deferred).
    pub bonus_drop_chance: i32,

    // --- Slice 5c T2: `Game::CreateBonus` constants (game.cpp:216-265) --------
    // The bonus-spawn search/draw inputs `create_bonus` reads once a bonus-drop
    // roll fires. Like the blood consts they are **defaulted (0/false/empty)**
    // here and assigned the real TC values by the difftest AFTER `new` (NO
    // `SimState::new` signature change) — left at the defaults `create_bonus` is
    // never reached (the roll is gated on `max_bonuses > 0`, which slices 1-5b
    // never set), so those goldens stay byte-identical. **None are hashed.**
    /// C++ `LC(BonusSpawnRectW)` (`common.c[CBonusSpawnRectW]`): the bound of the
    /// per-trial `rand(BonusSpawnRectW)` x-placement draw (`game.cpp:224`).
    pub bonus_spawn_rect_w: i32,
    /// C++ `LC(BonusSpawnRectH)`: the bound of the per-trial `rand(BonusSpawnRectH)`
    /// y-placement draw (`game.cpp:225`).
    pub bonus_spawn_rect_h: i32,
    /// C++ `LC(BonusSpawnRectX)`: the x-offset added to the placement **only** when
    /// the `HBonusSpawnRect` hack is set (`game.cpp:228`). Inert in this TC (hack off).
    pub bonus_spawn_rect_x: i32,
    /// C++ `LC(BonusSpawnRectY)`: the y-offset added when `HBonusSpawnRect` is set
    /// (`game.cpp:229`). Inert in this TC.
    pub bonus_spawn_rect_y: i32,
    /// C++ `common.h[HBonusSpawnRect]`: when set, offset the placement by
    /// `BonusSpawnRectX/Y` (`game.cpp:227`). False in the openliero TC.
    pub h_bonus_spawn_rect: bool,
    /// C++ `common.h[HBonusOnlyHealth]`: when set, force `frame = 1` (a health
    /// bonus) instead of drawing `rand(2)` (`game.cpp:235`). False in this TC.
    pub h_bonus_only_health: bool,
    /// C++ `common.h[HBonusOnlyWeapon]`: when set, force `frame = 0` (a weapon
    /// bonus) instead of drawing `rand(2)` (`game.cpp:237`). False in this TC.
    pub h_bonus_only_weapon: bool,
    /// C++ `common.h[HBonusDisable]`: when set, suppress the per-tick bonus-drop
    /// roll entirely (`game.cpp:359`, the `!h[HBonusDisable] && …` gate). False in
    /// the openliero TC; threaded for dumper symmetry (the T0 review flagged the
    /// missing term). Folded into [`process_frame`]'s roll gate.
    pub h_bonus_disable: bool,
    /// C++ `common.bonus_rand_timer[NUM_BONUS_SOBJECTS][2]` (`common.hpp:168`):
    /// per-`frame` `[base, range]`, where the spawn timer is `rand(range) + base`
    /// (`game.cpp:252`). `NUM_BONUS_SOBJECTS == 2`, so the two rows are frames 0
    /// (weapon) and 1 (health). Defaulted to zeros; the difftest assigns the TC's
    /// `constants.bonuses[i].{timer, timer_v}`.
    pub bonus_rand_timer: [[i32; 2]; 2],
    /// C++ `settings->weap_table` (`settings.hpp`): the per-weapon availability
    /// table (0/1/2). The weapon-bonus reject loop (`game.cpp:256-258`) re-draws
    /// `rand(weapons.size())` while `weap_table[w] == 2` (a banned weapon).
    /// Defaulted empty; the difftest assigns the real per-weapon table. Read only
    /// for a `frame == 0` bonus (when `max_bonuses > 0`).
    pub weap_table: Vec<i32>,

    // --- Slice 5c T3: `Bonus::Process` constants (bonus.cpp:6-35) -------------
    // The fall/bounce/expire inputs the per-tick bonuses Process loop reads. Like
    // the T2 bonus constants they are **defaulted (0)** here and assigned the real
    // TC values by the difftest AFTER `new` (NO `SimState::new` signature change);
    // left at the defaults the bonuses pool is empty (slices 1-5b never spawn a
    // bonus), so the bonuses loop is a no-op and those goldens stay byte-identical.
    // **None are hashed.**
    /// C++ `LC(BonusGravity)` (`common.c[CBonusGravity]`, `bonus.cpp:17`): the
    /// per-tick `vel_y` add a bonus gets while standing over air.
    pub bonus_gravity: i32,
    /// C++ `LC(BonusBounceMul)` (`bonus.cpp:22`): the numerator of the bounce
    /// reflection `vel_y = -(vel_y * BounceMul) / BounceDiv`.
    pub bonus_bounce_mul: i32,
    /// C++ `LC(BonusBounceDiv)` (`bonus.cpp:22`): the (truncating) divisor of the
    /// bounce reflection.
    pub bonus_bounce_div: i32,
    /// C++ `common.bonus_s_objects[NUM_BONUS_SOBJECTS]` (`common.hpp:170`): the
    /// per-`frame` expiry-sobject index. On `--timer<=0` the bonus spawns
    /// `sobject_types[bonus_s_objects[frame]]` (`bonus.cpp:30`). `NUM_BONUS_SOBJECTS
    /// == 2` (frames 0=weapon, 1=health). Defaulted to zeros; the difftest assigns
    /// the TC's `constants.bonuses[i].sound`-equivalent expiry-object index.
    pub bonus_s_objects: [i32; 2],

    /// C++ `Worm::settings->health` (`WormSettings::health{100}`, `worm.hpp:104`):
    /// the per-worm max/reset health. The clamp `health = min(health,
    /// settings->health)` (`worm.cpp:213`) caps every worm to it each tick, and
    /// `DoRespawning` (5d T5) restores `health = settings->health` on respawn. The
    /// oracle dumper never overrides `worm_settings[idx]->health`, so BOTH worms
    /// use the default **100** — a single scalar suffices for bit-exactness. NOT
    /// in the `new` arg list: defaulted to 100 (the C++ default) post-`new` — like
    /// the blood/bonus consts — so every existing call site is unchanged and the
    /// clamp is identity for slices 1-5c (health starts at 100 and never exceeds
    /// it), keeping those goldens byte-identical. **Not hashed** (settings scalar).
    pub settings_health: i32,

    /// C++ `Game::last_killed_idx` (`game.hpp`, default `-1`): the index of the
    /// most recently killed worm. Written by the death block (`worm.cpp:393-401`)
    /// and read by the GameOfTag "it"-transfer guard. **Not hashed** (game-level
    /// scalar, in neither the master nor the component fold); modelled only for a
    /// faithful port of the `:393-405` bookkeeping. Defaulted to `-1` post-`new`.
    pub last_killed_idx: i32,
    /// C++ `Game::got_changed` (`game.hpp`, default `false`): set by the death
    /// block to `old_last_killed != last_killed_idx` (`worm.cpp:401`), feeding
    /// GameOfTag / stats. **Not hashed**; defaulted to `false` post-`new`.
    pub got_changed: bool,

    // --- Slice 5d T4: `BeginRespawn`/`CheckRespawnPosition` constants ----------
    // (`worm.cpp:711-742`, `game.cpp:611-650`). The level-reading respawn-position
    // search reads these once the dead-arm countdown hits 0. Like the blood/bonus
    // consts they are **defaulted (0)** here and assigned the real TC values by
    // the difftest AFTER `new` (NO `SimState::new` signature change) — left at 0
    // they are inert because `begin_respawn` is only reached when a worm dies and
    // its `killed_timer` counts down to 0 (unreached for slices 1-5c, whose worms
    // never die), so those goldens stay byte-identical. **None are hashed.**
    /// C++ `LC(WormSpawnRectX)` (`common.c[CWormSpawnRectX]`, `worm.cpp:726`): the
    /// x-offset added to the per-trial `rand(WormSpawnRectW)` candidate x. Real TC
    /// value 5.
    pub worm_spawn_rect_x: i32,
    /// C++ `LC(WormSpawnRectY)` (`worm.cpp:727`): the y-offset added to the
    /// per-trial `rand(WormSpawnRectH)` candidate y. Real TC value 5.
    pub worm_spawn_rect_y: i32,
    /// C++ `LC(WormSpawnRectW)` (`worm.cpp:726`): the bound of the per-trial
    /// `rand(WormSpawnRectW)` x draw — **drawn FIRST** each trial. Real TC value 494.
    pub worm_spawn_rect_w: i32,
    /// C++ `LC(WormSpawnRectH)` (`worm.cpp:727`): the bound of the per-trial
    /// `rand(WormSpawnRectH)` y draw — **drawn SECOND** each trial. Real TC value 340.
    pub worm_spawn_rect_h: i32,
    /// C++ `LC(WormMinSpawnDistLast)` (`game.cpp:619-620`): the reject radius around
    /// the last-death position in `CheckRespawnPosition`. Real TC value 160.
    pub worm_min_spawn_dist_last: i32,
    /// C++ `LC(WormMinSpawnDistEnemy)` (`game.cpp:621-622`): the reject radius around
    /// the live enemy position in `CheckRespawnPosition`. Real TC value 160.
    pub worm_min_spawn_dist_enemy: i32,
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
    ///
    /// `h_signed_recoil` is the TC `[hacks].SignedRecoil` flag, threaded onto the
    /// state for the Fire path's recoil step (slices 1-3 never fire, so it is
    /// inert there; tests that do not fire pass `false`).
    ///
    /// `large_sprites` + `textures` are the assets Slice-4b Task 1's
    /// `DrawDirtEffect` reads (the 16x16 sprite bank and the TC texture table).
    /// Neither is hashed; slices 1-4a never index them (no dig), so they pass an
    /// empty `SpriteSet`/`Vec` and the goldens stay byte-identical. (The arg list
    /// is long; a builder is noted for later — see the prior reviews.)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        level: &LevelData,
        worms_init: &[WormInit],
        seed: u32,
        material_flags: &[u8; 256],
        weapons: Vec<Weapon>,
        physics: PhysicsConsts,
        control: ControlConsts,
        h_signed_recoil: bool,
        large_sprites: SpriteSet,
        textures: Vec<Texture>,
        sobject_types: Vec<SObjectType>,
        nobject_types: Vec<NObjectType>,
        settings_loading_time: i32,
        load_change: bool,
        blood: i32,
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
            control,
            weapons,
            // Built deterministically from the integer Taylor-series table; the
            // caller never supplies it (it is TC-independent).
            cossin: precompute_cossin(),
            h_signed_recoil,
            large_sprites,
            // Defaulted (empty); the differential harness assigns the real 7x7 bank
            // after `new`. Kept out of the arg list so existing call sites are
            // unchanged (only Slice-4d's shell-landing blit indexes it).
            small_sprites: SpriteSet::default(),
            textures,
            sobject_types,
            nobject_types,
            settings_loading_time,
            load_change,
            blood,
            // TC blood constants: defaulted (0); the differential harness assigns the
            // real values (`common.c[...]`) after `new`, the same post-`new` pattern
            // as `small_sprites`. Inert for slices 1-5a (no bobjects spawn there).
            num_blood_colours: 0,
            first_blood_colour: 0,
            bobj_gravity: 0,
            // Bonus-drop roll inputs: defaulted (0). Left at 0 the roll short-circuits
            // (NO rand) so slices 1-5b stay byte-identical; the difftest assigns the
            // real `max_bonuses`/`BonusDropChance` after `new` (post-`new` pattern, like
            // the blood consts) for the 5c scenario that makes the bonus pool live.
            settings_max_bonuses: 0,
            bonus_drop_chance: 0,
            // CreateBonus constants: defaulted (0/false/empty). create_bonus is only
            // reached when `max_bonuses > 0` (slices 1-5b never set it), so these stay
            // inert and the priors stay byte-identical; the difftest assigns the real
            // TC values after `new` (post-`new` pattern, like the blood consts).
            bonus_spawn_rect_w: 0,
            bonus_spawn_rect_h: 0,
            bonus_spawn_rect_x: 0,
            bonus_spawn_rect_y: 0,
            h_bonus_spawn_rect: false,
            h_bonus_only_health: false,
            h_bonus_only_weapon: false,
            h_bonus_disable: false,
            bonus_rand_timer: [[0, 0], [0, 0]],
            weap_table: Vec::new(),
            // Bonus::Process constants: defaulted (0). Left at 0 the bonuses pool is
            // empty (the drop roll never fires for slices 1-5b), so the bonuses loop
            // is a no-op and the priors stay byte-identical; the difftest assigns the
            // real TC values after `new` (post-`new` pattern, like the blood consts).
            bonus_gravity: 0,
            bonus_bounce_mul: 0,
            bonus_bounce_div: 0,
            bonus_s_objects: [0, 0],
            // Worm settings health: the C++ `WormSettings::health` default (100),
            // which the dumper never overrides. Post-`new` default (like the blood
            // consts) so no call site changes; the clamp is identity for slices
            // 1-5c (worms start at 100, never exceed it) => priors byte-identical.
            settings_health: 100,
            // Game-level kill bookkeeping (worm.cpp:393-401). C++ defaults:
            // last_killed_idx = -1, got_changed = false. Not hashed; written only
            // when a worm dies (unreached for slices 1-5c) => priors identical.
            last_killed_idx: -1,
            got_changed: false,
            // BeginRespawn/CheckRespawnPosition constants: defaulted (0). Left at 0
            // begin_respawn is unreached (no worm dies in slices 1-5c), so the
            // priors stay byte-identical; the difftest assigns the real TC values
            // (`tc.constants.WormSpawnRect*`/`WormMinSpawnDist*`) after `new`
            // (post-`new` pattern, like the blood consts).
            worm_spawn_rect_x: 0,
            worm_spawn_rect_y: 0,
            worm_spawn_rect_w: 0,
            worm_spawn_rect_h: 0,
            worm_min_spawn_dist_last: 0,
            worm_min_spawn_dist_enemy: 0,
        }
    }

    /// Advance one tick: a **subset** of `Game::ProcessFrame` (`game.cpp:333-355`
    /// object loops, then `++cycles` at `game.cpp:357`, then `worm.cpp:210-353` per
    /// worm) — *not* yet the whole frame (no bonus-drop RNG roll, no ninjarope
    /// `Process`, no `ProcessSight`; those land in a later slice). `cycles` advances
    /// once per tick at the exact game.cpp:357 point (after the object loops, before
    /// the worm loop); it folds into the master hash only (hash.rs:50), not the
    /// components. Renamed from the Slice-3 `process_worms` now that it runs the
    /// object-Process loops too.
    ///
    /// **Object loops run BEFORE the worm loop**, mirroring `Game::ProcessFrame`'s
    /// order (`game.cpp:334-355`): `sobjects` (no-op this slice), then the
    /// **wobjects** walk (the ported projectile per-tick `Process`), then
    /// `nobjects`/`bobjects` (no-ops). The wobjects walk steps slots in index
    /// order (mirroring `ExactObjectList::All()`): each live slot is copied out,
    /// run through [`wobject_process`], then written back ([`Keep`]) or freed
    /// ([`Explode`] -> [`blow_up`] + `free`, [`Remove`] -> `free`).
    ///
    /// **The load-bearing off-by-one:** because the wobjects loop runs *before*
    /// the worm loop, a shot spawned by [`worm_fire`] (inside the worm loop) is
    /// NOT visited on its birth tick — its first `pos` advance is the *next* tick.
    /// Walking by index (not [`Pool::iter`]) is what lets us free a slot mid-walk
    /// and skip a shot spawned later this same tick.
    ///
    /// [`Keep`]: WObjectOutcome::Keep
    /// [`Explode`]: WObjectOutcome::Explode
    /// [`Remove`]: WObjectOutcome::Remove
    ///
    /// **Input interleave** (matches the C++ dumper `sim_physics_dump.cpp:233-238`):
    /// for each worm in `worms` order, overwrite its `control_states` from the
    /// tick's input (mirroring `ControlState::Unpack`), then run that worm's full
    /// pass before moving to the next worm. Inputs shorter than `worms` leave the
    /// remaining worms' control state unchanged.
    ///
    /// Per-worm order (design doc, *Per-worm pass: exact ordering*):
    ///
    /// 1. `health = min(health, settings_health)` — inert this slice (no healing;
    ///    `health` starts at `settings_health` and never exceeds it), so skipped.
    /// 2. [`worm_reactions`] → `reacts` (may nudge `pos.y`/`vel.y`). Computed
    ///    **once** and read by BOTH `process_tasks` (jump) AND `worm_process_physics`
    ///    — never recomputed between (load-bearing).
    /// 3. `process_steerables` — no-op (empty `wobjects`).
    /// 4. movable reset.
    /// 5. [`process_aiming`].
    /// 6. [`process_tasks`] — jump reads `reacts[kRfUp]` and writes `vel.y`
    ///    **before** physics reads it (step 9).
    /// 7. [`process_weapons`].
    /// 8. *(Fire gate — OUT, Slice 4.)*
    /// 9. [`worm_process_physics`] — reads the SAME `reacts`.
    /// 10. *(ProcessSight — OUT, omitted.)*
    /// 11. Change gate: held → [`process_weapon_change`]; else clear
    ///     `key_change_pressed` + [`process_movement`] (walk writes `vel.x`
    ///     **after** physics, so it affects *next* tick's integration).
    pub fn process_frame(&mut self, inputs: &[ControlState]) {
        // Disjoint field borrows: destructuring `&mut self` binds each field as a
        // separate `&mut` (default binding mode), so the object loops can hold
        // `&mut wobjects`/`&mut rand` + `&weapons` while the worm loop separately
        // holds a `&mut` into `worms` and the Fire gate borrows the *other* fields
        // — all provably disjoint, which is what makes this borrow-check.
        let SimState {
            level,
            physics,
            control,
            worms,
            wobjects,
            sobjects,
            nobjects,
            bobjects,
            rand,
            weapons,
            cossin,
            h_signed_recoil,
            large_sprites,
            small_sprites,
            textures,
            sobject_types,
            nobject_types,
            settings_loading_time,
            load_change,
            blood,
            num_blood_colours,
            first_blood_colour,
            bobj_gravity,
            settings_max_bonuses,
            bonus_drop_chance,
            bonus_spawn_rect_w,
            bonus_spawn_rect_h,
            bonus_spawn_rect_x,
            bonus_spawn_rect_y,
            h_bonus_spawn_rect,
            h_bonus_only_health,
            h_bonus_only_weapon,
            h_bonus_disable,
            bonus_rand_timer,
            weap_table,
            bonus_gravity,
            bonus_bounce_mul,
            bonus_bounce_div,
            bonus_s_objects,
            bonuses,
            cycles,
            settings_health,
            last_killed_idx,
            got_changed,
            worm_spawn_rect_x,
            worm_spawn_rect_y,
            worm_spawn_rect_w,
            worm_spawn_rect_h,
            worm_min_spawn_dist_last,
            worm_min_spawn_dist_enemy,
            ..
        } = self;
        let h_signed_recoil = *h_signed_recoil;
        let settings_loading_time = *settings_loading_time;
        let load_change = *load_change;
        let blood = *blood;
        let num_blood_colours = *num_blood_colours;
        let first_blood_colour = *first_blood_colour;
        let bobj_gravity = *bobj_gravity;
        let settings_max_bonuses = *settings_max_bonuses;
        let bonus_drop_chance = *bonus_drop_chance;
        let bonus_spawn_rect_w = *bonus_spawn_rect_w;
        let bonus_spawn_rect_h = *bonus_spawn_rect_h;
        let bonus_spawn_rect_x = *bonus_spawn_rect_x;
        let bonus_spawn_rect_y = *bonus_spawn_rect_y;
        let h_bonus_spawn_rect = *h_bonus_spawn_rect;
        let h_bonus_only_health = *h_bonus_only_health;
        let h_bonus_only_weapon = *h_bonus_only_weapon;
        let h_bonus_disable = *h_bonus_disable;
        let bonus_gravity = *bonus_gravity;
        let bonus_bounce_mul = *bonus_bounce_mul;
        let bonus_bounce_div = *bonus_bounce_div;
        let settings_health = *settings_health;
        let worm_spawn_rect_x = *worm_spawn_rect_x;
        let worm_spawn_rect_y = *worm_spawn_rect_y;
        let worm_spawn_rect_w = *worm_spawn_rect_w;
        let worm_spawn_rect_h = *worm_spawn_rect_h;
        let worm_min_spawn_dist_last = *worm_min_spawn_dist_last;
        let worm_min_spawn_dist_enemy = *worm_min_spawn_dist_enemy;
        // The object loops read `cycles` as a value for the `cycles % delay` /
        // `cycles & 7` gates inside `nobject_process`. They must see the value left by
        // the PREVIOUS tick's increment (cycles=k-1 on tick k) — exactly as the C++
        // object loops run BEFORE `++cycles` (game.cpp:357). So snapshot the value
        // here, run the loops with it, then `++cycles` after the loops (see below).
        let cycles_now = *cycles;

        // ----- Bonuses Process loop (game.cpp:287-290), at the TOP of the tick,
        // BEFORE the object loops AND before `++cycles`. `bonuses` is an
        // ExactObjectList (slot order; All() skips free slots); `Bonus::Process`
        // (the fall/bounce/expire port) may Free(this) on the expire path. The
        // slot-walk in [`crate::bonus::process_bonuses`] copies each live bonus out
        // by value, runs it, then writes back (Keep) or frees the slot (Free, the
        // used-gated expire) — mirroring the existing object slot-walks. The expiry
        // sobject_create draws RNG; the fall/bounce path draws none. For slices 1-5b
        // the pool is EMPTY (no bonus ever spawns), so this loop is a NO-OP ⇒ those
        // goldens stay byte-identical. The constants default to 0 (unhashed) and are
        // inert until a 5c scenario makes the pool live.
        crate::bonus::process_bonuses(
            bonuses,
            level,
            worms,
            wobjects,
            nobjects,
            sobjects,
            weapons,
            nobject_types,
            sobject_types,
            cossin,
            large_sprites,
            textures,
            blood,
            bonus_gravity,
            bonus_bounce_mul,
            bonus_bounce_div,
            bonus_s_objects,
            rand,
        );

        // ----- Object Process loops (game.cpp:334-355), BEFORE the worm loop. --
        // sobjects: the ported SObject::Process (animation + free), FIRST. Walk
        // slots in INDEX order (== ExactObjectList::All()). Because this loop runs
        // BEFORE the wobjects loop, an sobject spawned LATER this tick (by
        // `blow_up` in the wobjects-loop Explode arm) is NOT visited — its first
        // animation step is next tick (game.cpp:334-337 precedes :339-342). No rand.
        for slot in 0..sobjects.capacity() {
            let obj_ref = match sobjects.get(slot) {
                Some(o) => o,
                None => continue,
            };
            let mut obj = *obj_ref;
            let ty = &sobject_types[obj.id as usize];
            match sobject_process(&mut obj, ty) {
                SObjectOutcome::Keep => {
                    *sobjects.get_mut(slot).expect("slot still live") = obj;
                }
                SObjectOutcome::Free => {
                    sobjects.free(slot);
                }
            }
        }
        // wobjects: the ported projectile per-tick Process. Walk slots in INDEX
        // order (== ExactObjectList::All()); freeing a slot mid-walk is safe, and
        // a wobject spawned LATER this tick (by Fire) is not visited.
        for slot in 0..wobjects.capacity() {
            let obj_ref = match wobjects.get(slot) {
                Some(o) => o,
                None => continue,
            };
            let mut obj = *obj_ref;
            let weapon = &weapons[obj
                .ty
                .expect("live wobject must carry a resolved weapon type")
                as usize];
            match wobject_process(&mut obj, level, weapon, cycles_now, rand) {
                WObjectOutcome::Keep => {
                    *wobjects.get_mut(slot).expect("slot still live") = obj;
                }
                WObjectOutcome::Explode => {
                    blow_up(
                        weapon,
                        level,
                        large_sprites,
                        textures,
                        obj.pos,
                        obj.owner_idx,
                        sobject_types,
                        nobject_types,
                        cossin,
                        worms,
                        wobjects,
                        weapons,
                        nobjects,
                        sobjects,
                        blood,
                        rand,
                    );
                    wobjects.free(slot);
                }
                WObjectOutcome::Remove => {
                    wobjects.free(slot);
                }
            }
        }
        // nobjects: the ported NObject::Process, THIRD (game.cpp:344-347). Walk
        // slots in INDEX order. Because this loop runs AFTER the wobjects loop,
        // dirt-debris spawned THIS tick (by `blow_up` -> `sobject_create`'s
        // dirt-throw, during the wobjects loop) ARE processed on their birth tick:
        // combined with `Create2`'s own birth `pos += vel`, that is the load-bearing
        // "double-step". Copy-out-by-value lets `nobject_process` take `&mut nobjects`
        // (its splinter-spawn arm) while `obj` is a local copy — same pattern as the
        // wobjects loop handing `&mut wobjects` to `blow_up`. The explode arms run
        // INSIDE `nobject_process`; the driver only frees on Explode/Remove.
        for slot in 0..nobjects.capacity() {
            let obj_ref = match nobjects.get(slot) {
                Some(o) => o,
                None => continue,
            };
            let mut obj = *obj_ref;
            let ty = &nobject_types[obj
                .ty
                .expect("live nobject must carry a resolved type") as usize];
            match nobject_process(
                &mut obj,
                ty,
                nobject_types,
                sobject_types,
                level,
                cossin,
                large_sprites,
                small_sprites,
                textures,
                worms,
                wobjects,
                weapons,
                nobjects,
                sobjects,
                bobjects,
                cycles_now,
                blood,
                num_blood_colours,
                first_blood_colour,
                rand,
            ) {
                NObjectOutcome::Keep => {
                    *nobjects.get_mut(slot).expect("slot still live") = obj;
                }
                NObjectOutcome::Explode | NObjectOutcome::Remove => {
                    nobjects.free(slot);
                }
            }
        }
        // bobjects: the ported BObject::Process, FOURTH (game.cpp:349-354). The
        // `FastObjectList` swap-remove-during-iteration loop — `if Process() ++i else
        // Free(i)` — is reproduced by `BloodPool::retain_processing` (keep == Process
        // returned true). Freeing a non-final slot moves the last live particle into
        // it and re-examines that slot WITHOUT advancing, so the surviving-slot order
        // (the entire bobject hash contract — pos-only fold) matches C++ exactly. Runs
        // AFTER the nobjects loop (so a bobject the blood-trail spawned THIS tick is
        // visited next tick, not this one) and BEFORE `++cycles`.
        bobjects.retain_processing(|obj| {
            crate::bobject::bobject_process(obj, level, bobj_gravity, rand)
        });

        // `++cycles` at the exact `game.cpp:357` point — AFTER the four object loops,
        // BEFORE the worm loop. The object loops above ran with `cycles_now` (the
        // value left by tick k-1's increment); after this the worm loop and the
        // tick-end master hash see cycles=k. `cycles` folds into the master
        // `HashGameState` only (hash.rs:50), NOT into any component hash, so advancing
        // it perturbs only the master column of the goldens. Must match the C++ dumper
        // exactly — the off-by-one is load-bearing for the `cycles % delay` gates read
        // DURING the object loop.
        *cycles = cycles.wrapping_add(1);

        // Bonus-drop roll (`game.cpp:359-362`), at the exact game.cpp:359 point — AFTER
        // `++cycles`, BEFORE the worm loop. The full C++ gate is
        // `!h[HBonusDisable] && max_bonuses > 0 && rand(CBonusDropChance) == 0` and the
        // `&&` short-circuits left-to-right: `HBonusDisable` set or `max_bonuses == 0`
        // both draw NO rand. THE WIN vs 5b: every prior scenario leaves `max_bonuses` at
        // its default 0, so the roll never fires and slices 1-5b stay byte-identical (no
        // golden regen). `HBonusDisable` is false in the openliero TC (so it never gates
        // here in practice) but the term is threaded for dumper symmetry — T0's review
        // flagged its absence. The `rand(CBonusDropChance)` draw uses the SAME shared RNG
        // at this load-bearing position; on a 0 roll it runs `create_bonus` (T2 port).
        if !h_bonus_disable
            && settings_max_bonuses > 0
            && rand.bound(bonus_drop_chance as u32) == 0
        {
            crate::bonus::create_bonus(
                bonuses,
                level,
                worms,
                wobjects,
                nobjects,
                sobjects,
                weapons,
                nobject_types,
                sobject_types,
                cossin,
                large_sprites,
                textures,
                blood,
                settings_max_bonuses,
                bonus_spawn_rect_w,
                bonus_spawn_rect_h,
                bonus_spawn_rect_x,
                bonus_spawn_rect_y,
                h_bonus_spawn_rect,
                h_bonus_only_health,
                h_bonus_only_weapon,
                bonus_rand_timer,
                weap_table,
                rand,
            );
        }

        // Deferred kill attribution (worm.cpp:403-405). A dying worm's death
        // block increments the KILLER's `kills` — a *different* worm the in-loop
        // `&mut` borrow cannot touch. Collect the killer indices here and apply
        // them AFTER the worm loop; hash-equivalent because `kills` is read only
        // at the end-of-tick fold (nothing in the worm body branches on it).
        let mut deferred_kills: Vec<usize> = Vec::new();

        // Index-based (not `iter_mut()`): the dead arm's `begin_respawn` reads the
        // LIVE enemy worm `worms[i ^ 1].pos` at worm `i`'s turn — a second element
        // of the same slice an `iter_mut()` `&mut` borrow could not reach. Indexing
        // reborrows `worms` per access, so the enemy read and the self-mutation are
        // both expressible. The visible arm rebinds `let w = &mut worms[i]` and is
        // otherwise unchanged.
        for i in 0..worms.len() {
            // Interleave: apply this worm's input (≈ `Unpack`), then Process it.
            if let Some(input) = inputs.get(i) {
                worms[i].control_states = *input;
            }

            // Port of `Worm::Process` (worm.cpp:210-452). The C++ structure is:
            //   health = min(health, settings_health);          // 213 — ALWAYS
            //   if ((mode != KillEmAll && mode != Scales) || lives > 0) {  // 215
            //     if (visible) { ...active-sim body (steps 2-11)... }      // 218
            //     else { steerable_count = 0; PressedOnce(kFire)->ready;   // 431-450
            //            --killed_timer; BeginRespawn; DoRespawning; }
            //   }

            // Health clamp (worm.cpp:213) — ALWAYS, BEFORE the game-mode/lives
            // gate, so it caps even a gate-closed (lives==0) worm. Identity for
            // slices 1-5c (worms start at settings_health == 100 and never exceed
            // it), so priors stay byte-identical.
            worms[i].health = worms[i].health.min(settings_health);

            // Game-mode / lives gate (worm.cpp:215). The full C++ condition is
            // `(mode != KillEmAll && mode != Scales) || lives > 0`; the openliero
            // TC mode is KillEmAll (and Scales folds the same way), so it reduces
            // to `lives > 0`. Non-KillEmAll/Scales modes (e.g. GameOfTag) would
            // make the gate always-true — those branches stay present-but-guarded
            // (game_mode is unmodelled; the TC is always KillEmAll). Hash-neutral
            // for priors (lives > 0 always in 1-5c).
            if worms[i].lives <= 0 {
                continue;
            }

            if worms[i].visible {
                // Rebind the per-worm `&mut` for the (unchanged) visible arm. Held
                // only within this arm; the dead arm indexes `worms` directly so
                // `begin_respawn` can also read the enemy slot.
                let w = &mut worms[i];
                // 2. reaction orchestration -> reacts (shared by tasks + physics).
                let reacts = worm_reactions(level, w, physics);

                // 3. process_steerables: no-op this slice (empty wobjects).

                // 4. movable reset (worm.cpp:330-333).
                if !w.movable
                    && !w.control_states.get(ControlState::LEFT)
                    && !w.control_states.get(ControlState::RIGHT)
                {
                    w.movable = true;
                }

                // 5. aiming.
                process_aiming(w, control);

                // 6. tasks (jump reads reacts[kRfUp], writes vel.y BEFORE physics).
                process_tasks(w, &reacts, control);

                // 7. weapons (delay_left countdown + shell drop on timer expiry).
                process_weapons(
                    w,
                    rand,
                    nobjects,
                    nobject_types,
                    i as i32,
                    weapons,
                    settings_loading_time,
                );

                // 8. Fire gate (worm.cpp:336-339), ported verbatim: Fire held,
                //    Change NOT held, the current slot Available() (loading_left ==
                //    0) and its delay_left <= 0. (No `ammo > 0` term — C++ has none.)
                let cw = w.current_weapon as usize;
                if w.control_states.get(ControlState::FIRE)
                    && !w.control_states.get(ControlState::CHANGE)
                    && w.weapons[cw].available()
                    && w.weapons[cw].delay_left <= 0
                {
                    worm_fire(w, weapons, cossin, h_signed_recoil, rand, wobjects);
                }

                // 9. physics — reads the SAME reacts computed in step 2.
                worm_process_physics(w, &reacts, physics);

                // 10. ProcessSight — omitted.

                // 11. change/movement gate (worm.cpp:348-353).
                if w.control_states.get(ControlState::CHANGE) {
                    process_weapon_change(w, load_change);
                } else {
                    w.key_change_pressed = false;
                    process_movement(
                        w,
                        control,
                        level,
                        large_sprites,
                        textures,
                        cossin,
                        rand,
                    );
                }

                // 12. Pre-death blood drip (worm.cpp:355-367) — fires at the END
                //     of the visible arm (after the change/movement gate) while
                //     the worm is alive but under settings_health/4. Hash-neutral
                //     for slices 1-5c: their worms start at full health
                //     (>= settings_health/4) so the outer gate never opens and no
                //     rand is drawn (goldens stay byte-identical).
                worm_pre_death_drip(w, i as i32, settings_health, nobject_types, rand, nobjects);

                // 13. Death block (worm.cpp:369-426) — fires at the very END of
                //     the visible arm when `health <= 0`. Plays a death sound
                //     (rand(3)), decrements `lives`, hides the worm, arms
                //     `killed_timer = 150`, and sprays the kMax-blood fan + 8
                //     worm-gibs. Returns the killer index (if any) to defer its
                //     `kills++`. Inert for slices 1-5c (worms never reach
                //     health <= 0), so those goldens stay byte-identical.
                if let Some(killer) = worm_death(
                    w,
                    i as i32,
                    blood,
                    nobject_types,
                    cossin,
                    rand,
                    nobjects,
                    last_killed_idx,
                    got_changed,
                ) {
                    deferred_kills.push(killer);
                }
            } else {
                // Worm is dead (worm.cpp:431-450). None of this touches a hashed
                // field except `killed_timer` (unhashed) and — on a Fire hit —
                // `control_states` (the read-and-clear of the Fire bit); both are
                // unreached for slices 1-5c, whose worms are all visible, so those
                // goldens stay byte-identical.
                worms[i].steerable_count = 0;

                // PressedOnce(kFire) (worm.hpp:187-191): read the Fire bit, CLEAR
                // it, and set `ready` when it was set. `ready` gates the
                // `DoRespawning` completion (T5).
                let fire = worms[i].control_states.get(ControlState::FIRE);
                worms[i].control_states.set(ControlState::FIRE, false);
                if fire {
                    worms[i].ready = true;
                }

                // killed_timer countdown (worm.cpp:439-449). The 150-tick dead
                // phase is hash-silent (killed_timer is in NEITHER hash); the
                // countdown is pinned only transitively through WHEN the
                // BeginRespawn RNG burst lands.
                if worms[i].killed_timer > 0 {
                    worms[i].killed_timer -= 1;
                }
                // `killed_timer == 0 && !game.quick_sim` -> BeginRespawn
                // (worm.cpp:443). QUICK_SIM-GUARD DECISION: the sim does not model
                // `quick_sim`, and the oracle dumper drives the unmodified
                // `Worm::Process` with `Game::quick_sim{false}` (game.hpp:153, never
                // overridden), so the `!quick_sim` term is a compile-time-constant
                // `true` for every dumped tick. Rather than introduce an always-true
                // named-const branch (dead code a reader must reason about, and a
                // clippy hazard), the term is deliberately OMITTED and documented
                // here: were quick_sim ever modelled, this branch would need
                // `&& !quick_sim` re-added. The reads below (`killed_timer` fresh
                // each `if`) mirror C++ so BeginRespawn (which sets `killed_timer =
                // -1`) falls straight through into the `< 0` DoRespawning arm on the
                // SAME tick — exactly as `worm.cpp:443-449`.
                if worms[i].killed_timer == 0 {
                    begin_respawn(
                        worms,
                        i,
                        level,
                        worm_spawn_rect_x,
                        worm_spawn_rect_y,
                        worm_spawn_rect_w,
                        worm_spawn_rect_h,
                        worm_min_spawn_dist_last,
                        worm_min_spawn_dist_enemy,
                        rand,
                    );
                }
                // `killed_timer < 0` -> DoRespawning (worm.cpp:447). Ported in
                // Slice 5d T5: the drop-in convergence walk + the completion
                // (dirt puff, aiming reset, health restore) once the cursor
                // reaches `Ftoi(pos)-80` within ±5 AND `ready`.
                if worms[i].killed_timer < 0 {
                    do_respawning(
                        &mut worms[i],
                        level,
                        large_sprites,
                        textures,
                        settings_health,
                        rand,
                    );
                }
            }
        }

        // Apply the deferred killer `kills++` (worm.cpp:403-405) now that the
        // worm-loop `&mut` borrow is released. Order is irrelevant: `kills` is
        // additive and folded only at end-of-tick, so this matches the C++
        // in-place increment bit-for-bit in the hash.
        for killer in deferred_kills {
            worms[killer].kills += 1;
        }
    }
}

/// Port of `Worm::BeginRespawn` (`worm.cpp:711-742`) — the level-reading RNG
/// respawn-position search, **the canonical Step-2 desync trap** (the trial count
/// = f(live level pixels, live enemy pos)).
///
/// RNG order (the contract, verified against `worm.cpp:711-742`):
/// 1. `temp = Ftoi(pos)` (the death pos, no rand); `logic_respawn = temp -
///    (80,80)` (`:714-716`); `enemy = temp`, then iff `worms.size() == 2`
///    `enemy = Ftoi(worms[index ^ 1].pos)` — the **LIVE enemy pos** read at THIS
///    worm's turn in the loop (a desync input, no rand) (`:718-722`);
/// 2. a `do { … } while (!CheckRespawnPosition(…))` trial loop (`:725-739`). Each
///    trial draws **EXACTLY 2** values in this order: `rand(WormSpawnRectW)`
///    **FIRST** (the candidate x, `:726`) **then** `rand(WormSpawnRectH)` (the
///    candidate y, `:727`). The candidate pos is `Itof(SpawnRectX + x)` /
///    `Itof(SpawnRectY + y)`;
/// 3. a drop-down `while (Ftoi(pos.y)+4 < height && Mat(x, y+4).Background())
///    pos.y += Itof(1)` (`:731-734`) — reads the LIVE level, draws **NO rand**;
/// 4. `if (++trials >= 50000) break;` (`:736-738`) — the runaway guard checked
///    BEFORE the while-condition;
/// 5. the `while` re-evaluates `CheckRespawnPosition` (also rand-free): accept
///    (`true` → `!true` exits) or reject (`false` → another trial);
/// 6. `killed_timer = -1` on exit (`:741`).
///
/// `worms` is the whole slice (indexed, not an `iter_mut()` borrow) so the enemy
/// read (`worms[index ^ 1]`) and the self-mutation (`worms[index]`) coexist.
#[allow(clippy::too_many_arguments)]
fn begin_respawn(
    worms: &mut [WormState],
    index: usize,
    level: &LevelSim,
    spawn_rect_x: i32,
    spawn_rect_y: i32,
    spawn_rect_w: i32,
    spawn_rect_h: i32,
    min_spawn_dist_last: i32,
    min_spawn_dist_enemy: i32,
    rand: &mut Rand,
) {
    // :714 temp = Ftoi(pos) — the death position, in integer pixels.
    let temp_x = ftoi(worms[index].pos.x);
    let temp_y = ftoi(worms[index].pos.y);

    // :716 logic_respawn = temp - IVec2(80, 80). Stored in the Vec2-as-IVec2 field
    // (integer pixels), the drop-in cursor DoRespawning (T5) walks.
    worms[index].logic_respawn = Vec2::new(temp_x - 80, temp_y - 80);

    // :718-722 enemy = temp; iff two worms, enemy = Ftoi(worms[index^1].pos) — the
    // LIVE enemy pos (a desync input). Only read when `worms.len() == 2`.
    let (mut enemy_x, mut enemy_y) = (temp_x, temp_y);
    if worms.len() == 2 {
        enemy_x = ftoi(worms[index ^ 1].pos.x);
        enemy_y = ftoi(worms[index ^ 1].pos.y);
    }

    // :724-739 the trial loop. `do { … } while (!Check…)` -> Rust `loop { … }`
    // with the break points transcribed in the exact C++ order.
    let mut trials: i32 = 0;
    loop {
        // :726 candidate x — `rand(WormSpawnRectW)` drawn FIRST.
        let cand_x = spawn_rect_x + rand.bound(spawn_rect_w as u32) as i32;
        // :727 candidate y — `rand(WormSpawnRectH)` drawn SECOND.
        let cand_y = spawn_rect_y + rand.bound(spawn_rect_h as u32) as i32;
        worms[index].pos.x = itof(cand_x);
        worms[index].pos.y = itof(cand_y);

        // :731-734 drop-down: slide `pos.y` down over Background pixels. Reads the
        // LIVE level via `Mat(x, y+4).Background()` (in-bounds; guarded by
        // `Ftoi(pos.y)+4 < height`), draws NO rand.
        while ftoi(worms[index].pos.y) + 4 < level.height
            && level.background(ftoi(worms[index].pos.x), ftoi(worms[index].pos.y) + 4)
        {
            worms[index].pos.y = worms[index].pos.y.wrapping_add(itof(1));
        }

        // :736-738 runaway guard — `++trials` then compare, BEFORE the while-cond.
        trials += 1;
        if trials >= 50000 {
            break;
        }

        // :739 the `while (!CheckRespawnPosition(…))` condition: accept -> break.
        let cx = ftoi(worms[index].pos.x);
        let cy = ftoi(worms[index].pos.y);
        if check_respawn_position(
            level,
            enemy_x,
            enemy_y,
            temp_x,
            temp_y,
            cx,
            cy,
            min_spawn_dist_last,
            min_spawn_dist_enemy,
        ) {
            break;
        }
    }

    // :741 killed_timer = -1 (hand off to DoRespawning next).
    worms[index].killed_timer = -1;
}

/// Port of `CheckRespawnPosition` (`game.cpp:611-650`) — the **rand-free** accept
/// test the `BeginRespawn` trial loop evaluates per candidate. Returns `true` to
/// accept the candidate `(x, y)`, `false` to reject (forcing another trial).
///
/// Reject order (verified against `game.cpp:614-647`):
/// 1. the min-distance rejects FIRST (`:619-624`): reject if within
///    `WormMinSpawnDistLast` of the last-death pos **OR** within
///    `WormMinSpawnDistEnemy` of the enemy;
/// 2. then the `Rock()` box scan (`:626-647`): reject on any rock pixel in
///    `[x-3, x+3) × [y-4, y+4)` (clamped to the level).
///
/// **C++ quirk mirrored faithfully (`game.cpp:614`):** `kDeltaX = old_x` — the
/// last-position x "delta" is the raw `old_x`, **NOT** `old_x - x`. So the
/// last-pos reject fires on the OLD x being near 0 (regardless of the candidate x)
/// and never fires for a large `old_x` even if the candidate sits on the old spot.
/// This is a genuine engine bug (the y term IS a real delta, `old_y - y`); the
/// port reproduces it bit-for-bit, and the `Rock()` "special rock respawn bug"
/// TODO (`game.cpp:642`) behaviour is likewise kept AS-IS, not "fixed".
#[allow(clippy::too_many_arguments)]
fn check_respawn_position(
    level: &LevelSim,
    x2: i32,
    y2: i32,
    old_x: i32,
    old_y: i32,
    x: i32,
    y: i32,
    min_spawn_dist_last: i32,
    min_spawn_dist_enemy: i32,
) -> bool {
    // :614-617 — NB `kDeltaX = old_x` (the raw last x, NOT `old_x - x`; C++ bug,
    // mirrored). `kDeltaY` IS a real delta.
    let k_delta_x = old_x;
    let k_delta_y = old_y - y;
    let k_enemy_dx = x2 - x;
    let k_enemy_dy = y2 - y;

    // :619-624 min-distance rejects FIRST (last-pos OR enemy).
    if (k_delta_x.abs() <= min_spawn_dist_last && k_delta_y.abs() <= min_spawn_dist_last)
        || (k_enemy_dx.abs() <= min_spawn_dist_enemy && k_enemy_dy.abs() <= min_spawn_dist_enemy)
    {
        return false;
    }

    // :626-638 the [x-3, x+3) × [y-4, y+4) box, clamped to the level.
    let mut max_x = x + 3;
    let mut max_y = y + 4;
    let mut min_x = x - 3;
    let mut min_y = y - 4;
    if max_x >= level.width {
        max_x = level.width - 1;
    }
    if max_y >= level.height {
        max_y = level.height - 1;
    }
    min_x = min_x.max(0);
    min_y = min_y.max(0);

    // :640-647 reject on any Rock() pixel (half-open `!=` bounds, exactly as C++;
    // the clamps guarantee `min <= max`). The "special rock respawn bug" TODO
    // (:642) behaviour is intentionally preserved.
    let mut i = min_x;
    while i != max_x {
        let mut j = min_y;
        while j != max_y {
            if level.rock(i, j) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }

    // :649 accept.
    true
}

/// Port of `LimitXy` (`worm.cpp:744-753`): clamp `(x, y)` into
/// `[0, max_x] × [0, max_y]` in place. The C++ x/y arms use different idioms
/// (`if/else if` vs `std::max`/`std::min`) but both clamp to `[0, max]`.
fn limit_xy(x: &mut i32, y: &mut i32, max_x: i32, max_y: i32) {
    if *x < 0 {
        *x = 0;
    } else if *x > max_x {
        *x = max_x;
    }
    *y = (*y).max(0).min(max_y);
}

/// Port of `Worm::DoRespawning` (`worm.cpp:755-809`) — run each tick while
/// `killed_timer < 0`. It walks the `logic_respawn` cursor toward
/// `Ftoi(pos) - 80` (the drop-in point), and once the cursor has converged
/// (within ±5) AND the player has pressed Fire (`ready`), it completes the
/// respawn: a dirt puff, the aiming reset, and (KillEmAll) the health restore.
///
/// RNG contract (the only two draws, in this order):
/// 1. [`draw_dirt_effect`]'s one `rand(rframe)` (drawn before any pixel write);
/// 2. the lone **no-arg** `rand() & 1` (`:799`) — the raw next MT draw's LOW bit.
///    C++ is `game.rand() & 1` (`Rand::operator()()` = `last = engine()`), so
///    this is [`Rand::next_u32`]`() & 1`, NOT `rand.bound(2)` (which would be the
///    HIGH bit of the same draw): they advance the RNG identically but select
///    different bits, so the call form is load-bearing.
///
/// Omissions (faithful to the dumper's settings): `CorrectShadow` (`:784-786`,
/// gated on `settings->shadow`, **false**), the `SoundAlive` play (`:789`) and
/// `AfterSpawn` stats (`:807`) — all render/sound/stats side effects the sim
/// drops. The Scales-of-Justice guard on the health restore (`:794`) folds away
/// (the TC is KillEmAll), so `health` is always restored here.
#[allow(clippy::too_many_arguments)]
fn do_respawning(
    worm: &mut WormState,
    level: &mut LevelSim,
    large_sprites: &SpriteSet,
    textures: &[Texture],
    settings_health: i32,
    rand: &mut Rand,
) {
    // :758-770 step the cursor toward Ftoi(pos)-80 by ±1, FOUR times per tick,
    // each axis independently. No rand. C++ re-reads Ftoi(pos) each iteration;
    // `pos` is constant across the loop, so recomputing the target here matches.
    for _ in 0..4 {
        let dest_x = ftoi(worm.pos.x) - 80;
        if worm.logic_respawn.x < dest_x {
            worm.logic_respawn.x += 1;
        } else if worm.logic_respawn.x > dest_x {
            worm.logic_respawn.x -= 1;
        }

        let dest_y = ftoi(worm.pos.y) - 80;
        if worm.logic_respawn.y < dest_y {
            worm.logic_respawn.y += 1;
        } else if worm.logic_respawn.y > dest_y {
            worm.logic_respawn.y -= 1;
        }
    }

    // :772 clamp the cursor into the level (a 158px bottom/right margin).
    limit_xy(
        &mut worm.logic_respawn.x,
        &mut worm.logic_respawn.y,
        level.width - 158,
        level.height - 158,
    );

    // :774-776 the (clamped) destination the cursor converges on.
    let mut dest_x = ftoi(worm.pos.x) - 80;
    let mut dest_y = ftoi(worm.pos.y) - 80;
    limit_xy(&mut dest_x, &mut dest_y, level.width - 158, level.height - 158);

    // :778-780 converged within ±5 of the destination AND `ready` (the Fire
    // press the dead arm latched; "Don't spawn in quicksim").
    if worm.logic_respawn.x < dest_x + 5
        && worm.logic_respawn.x > dest_x - 5
        && worm.logic_respawn.y < dest_y + 5
        && worm.logic_respawn.y > dest_y - 5
        && worm.ready
    {
        // :782-783 dirt puff at Ftoi(pos)-7 (dirt_effect 0). This consumes the
        // FIRST of the two draws (draw_dirt_effect draws before writing pixels).
        let ipos_x = ftoi(worm.pos.x);
        let ipos_y = ftoi(worm.pos.y);
        draw_dirt_effect(level, large_sprites, textures, 0, ipos_x - 7, ipos_y - 7, rand);

        // :784-786 CorrectShadow — gated on settings->shadow (false) => OMITTED.

        // :788 ready = false; :789 Play(SoundAlive) => sound-only, omitted.
        worm.ready = false;
        // :791-793 revive.
        worm.visible = true;
        worm.fire_cone = 0;
        worm.vel = Vec2::zero();
        // :794-796 health = settings->health (Scales guard folds away; KillEmAll).
        worm.health = settings_health;

        // :799-805 the lone no-arg `rand() & 1` (raw next draw's LOW bit). Odd =>
        // face left-ish (Itof(32), dir 0); even => face right-ish (Itof(96), dir 1).
        if rand.next_u32() & 1 != 0 {
            worm.aiming_angle = itof(32);
            worm.direction = 0;
        } else {
            worm.aiming_angle = itof(96);
            worm.direction = 1;
        }

        // :807 AfterSpawn(this) — stats, omitted.
    }
}

/// Port of the **pre-death blood drip** (`worm.cpp:355-367`) — the tail of the
/// visible arm that sprays a single blood `nobject` while the worm is alive but
/// under `settings_health / 4`.
///
/// RNG order (the contract, verified against `worm.cpp:355-367`):
/// 1. `rand(health + 6)` (`:356`, the OUTER gate);
/// 2. iff `== 0`: `rand(3)` (`:357`, the INNER gate);
/// 3. iff the inner `== 0`: `rand(3)` for the sound index `18 + …` (`:358-359`) —
///    **always drawn on the inner gate** (the C++ note pins it *outside* the
///    unpredictable `IsPlaying`/`Play` branch, which draws no rand and is a
///    sound-only side effect the sim omits);
/// 4. **unconditionally within the OUTER gate** (whenever the outer roll was 0,
///    regardless of the inner/sound gate): `nobject_types[6].Create1(vel, pos, 0,
///    index)` (`:365`) — the blood spawn, which itself draws `rand(dist*2)` twice
///    (x, y) when blood's `distribution != 0`.
///
/// The `Create1` spawn sits **inside** the outer gate but **outside** the sound
/// gate. Gated on `health < settings_health / 4` (integer `/`), so it is inert —
/// zero draws — for a full-health worm; slices 1-5c (worms at `settings_health`)
/// never open the gate, keeping their goldens byte-identical.
#[allow(clippy::too_many_arguments)]
fn worm_pre_death_drip(
    w: &WormState,
    index: i32,
    settings_health: i32,
    nobject_types: &[NObjectType],
    rand: &mut Rand,
    nobjects: &mut Pool<NObject>,
) {
    // :355 outer gate — integer `/4`, strict `<`.
    if w.health < settings_health / 4 {
        // :356 outer roll. `(health + 6) as u32` mirrors C++ `int -> uint32_t`
        // (2's-complement), matching `game.rand(health + 6)` for any health.
        if rand.bound((w.health + 6) as u32) == 0 {
            // :357 inner roll.
            if rand.bound(3) == 0 {
                // :358-359 sound index `18 + rand(3)`. The draw is kept (it
                // advances the shared engine and is pinned outside the
                // unpredictable IsPlaying branch); the Play side effect is
                // sound-only and omitted from the sim.
                let _snd = 18 + rand.bound(3);
            }
            // :365 Create1 is UNCONDITIONAL within the outer gate (outside the
            // sound gate). Blood is nobject_types[6]; color 0, owner = index.
            nobject_create1(&nobject_types[6], w.vel, w.pos, 0, index, rand, nobjects);
        }
    }
}

/// Port of the **death block** (`worm.cpp:369-426`) — the tail of the visible arm
/// that runs when a worm's `health` reaches `<= 0`: it plays a death sound,
/// decrements `lives`, does the kill bookkeeping, hides the worm, arms the
/// `killed_timer`, and sprays the `kMax`-particle blood fan + the 8 worm-gibs.
///
/// RNG order (the contract, verified against `worm.cpp:369-426`):
/// 1. `rand(3)` death-sound index `15 + …` (`:378`) — the ONLY pre-spray draw
///    (the `loop_sound` Stop and the `Play` are sound-only side effects with no
///    rand, omitted by the sim);
/// 2. `--lives` / kill bookkeeping (`:384-405`) — **no rand**;
/// 3. **iff `kMax = 120*blood/100 > 1`** (`:412`, strict `>`, NOT `>= 1`): for
///    `i in 1..=kMax` a `rand(128)` angle (`:414`, the arg — drawn in worm.cpp,
///    OUTSIDE `Create2`) then `nobject_types[6].Create2` (its own draws);
/// 4. the **8**-iteration gib spray `for i in (7..=105).step_by(14)`
///    (`{7,21,35,49,63,77,91,105}`, `:418`): a `rand(14)` angle (`:419`, added to
///    `i`, drawn OUTSIDE `Create2`) then `nobject_types[index].Create2` — the gib
///    type is the **per-worm** type `nobject_types[index]` (worm index 0/1), NOT
///    blood.
///
/// Returns `Some(killer_idx)` when the killer's `kills` must be incremented
/// (`:403-405`, `last_killed_by_idx >= 0 && != index`) — applied by the caller
/// AFTER the worm loop (the `kills++` targets a *different* worm, which the
/// in-loop `&mut` borrow forbids; deferring is hash-equivalent since `kills` is
/// read only at the end-of-tick fold). `None` otherwise. Gated on `health <= 0`
/// (`:369`), so it is inert — zero draws, no mutation — for a live worm; slices
/// 1-5c (worms at full health) never enter it, keeping their goldens
/// byte-identical.
#[allow(clippy::too_many_arguments)]
fn worm_death(
    w: &mut WormState,
    index: i32,
    blood: i32,
    nobject_types: &[NObjectType],
    cossin: &[Vec2; 128],
    rand: &mut Rand,
    nobjects: &mut Pool<NObject>,
    last_killed_idx: &mut i32,
    got_changed: &mut bool,
) -> Option<usize> {
    // :369 gate. Inert (zero draws, no mutation) while the worm is alive.
    if w.health > 0 {
        return None;
    }

    // :370-371 clear the shell-drop timer + the green-sight flag.
    w.leave_shell_timer = 0;
    w.make_sight_green = false;

    // :373-376 stop the current weapon's loop_sound — a sound-only side effect
    // with no rand; omitted (loop_sound is not modelled).

    // :378 death-sound index `15 + rand(3)`. This is the ONLY draw before the
    // sprays; the `Play` at :379 is a sound-only side effect the sim omits.
    let _death_snd = 15 + rand.bound(3);

    // :381-382 firecone off, rope stowed.
    w.fire_cone = 0;
    w.ninjarope.out = false;

    // :384-391 lives. KillEmAll: `--lives` (:390). The Scales branch
    // (`while health <= 0 { health += settings->health; --lives }`, :385-388) is
    // the unmodelled alternate — game_mode is not modelled and the TC is always
    // KillEmAll (see the T1 gate comment), so it is guarded-by-absence.
    w.lives -= 1;

    // :393-401 last_killed_idx / got_changed bookkeeping (no rand; unhashed).
    // The GameOfTag guard at :396-398 is always true outside GameOfTag, so in
    // KillEmAll `game.last_killed_idx = index` unconditionally.
    let old_last_killed = *last_killed_idx;
    *last_killed_idx = index;
    *got_changed = old_last_killed != *last_killed_idx;

    // :403-405 the killer's `kills++` (hashed on master). Deferred to the caller
    // (targets a *different* worm; the in-loop `&mut` borrow forbids touching it
    // here). Hash-equivalent: `kills` is read only at the end-of-tick fold.
    let killer = if w.last_killed_by_idx >= 0 && w.last_killed_by_idx != index {
        Some(w.last_killed_by_idx as usize)
    } else {
        None
    };

    // :407-408 hide the worm + arm the dead-phase countdown.
    w.visible = false;
    w.killed_timer = KILLED_TIMER_INITIAL;

    // :410 kMax = 120 * blood / 100 (integer `/`). :412 the blood spray fires
    // ONLY when kMax > 1 (strict — kMax == 1, i.e. blood == 1, draws nothing).
    let k_max = 120 * blood / 100;
    if k_max > 1 {
        for _ in 1..=k_max {
            // :414 rand(128) is the angle ARG (drawn in worm.cpp, evaluated
            // BEFORE Create2's own draws), then blood `nobject_types[6].Create2`
            // with vel/3 (truncating), color 0, owner = index.
            let angle = rand.bound(128) as i32;
            nobject_create2(
                &nobject_types[6],
                angle,
                w.vel.div(3),
                w.pos,
                0,
                index,
                cossin,
                rand,
                nobjects,
            );
        }
    }

    // :418-421 worm-gib spray — `for (i = 7; i <= 105; i += 14)` = EXACTLY 8
    // iterations {7,21,35,49,63,77,91,105}. The angle is `i + rand(14)` (the
    // rand drawn in worm.cpp, outside Create2); the gib type is the PER-WORM
    // type `nobject_types[index]` (worm index 0/1), NOT blood.
    for i in (7..=105).step_by(14) {
        let angle = i + rand.bound(14) as i32;
        nobject_create2(
            &nobject_types[index as usize],
            angle,
            w.vel.div(3),
            w.pos,
            0,
            index,
            cossin,
            rand,
            nobjects,
        );
    }

    // :423 AfterDeath is a StatsRecorder no-op in the base recorder.
    // :425 Release(kFire) clears the Fire bit — folded into the master hash via
    // control_states.pack(), so it must run on the death tick.
    w.control_states.set(ControlState::FIRE, false);

    killer
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
            WeaponInit {
                ty: Some(0),
                ammo: 10,
            },
            WeaponInit {
                ty: Some(1),
                ammo: 1,
            },
            WeaponInit {
                ty: Some(2),
                ammo: 50,
            },
            WeaponInit {
                ty: Some(3),
                ammo: 3,
            },
            WeaponInit {
                ty: Some(4),
                ammo: 25,
            },
        ];
        let weapons1 = [
            WeaponInit {
                ty: Some(5),
                ammo: 2,
            },
            WeaponInit {
                ty: Some(6),
                ammo: 8,
            },
            WeaponInit {
                ty: Some(7),
                ammo: 100,
            },
            WeaponInit {
                ty: Some(8),
                ammo: 4,
            },
            WeaponInit {
                ty: Some(9),
                ammo: 1,
            },
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
        let state = SimState::new(
            &level,
            &two_worms(),
            0x1234,
            &[0u8; 256],
            Vec::new(),
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
        assert_eq!(state.cycles, 0, "cycles must be 0 at tick 0");
        assert_eq!(state.rand.last(), 0, "no RNG consumed -> last() == 0");
        assert_eq!(state.level.width, 4);
        assert_eq!(state.level.height, 4);
        assert_eq!(
            state.level.material_id, level.material_id,
            "material map copied verbatim"
        );
        assert_eq!(state.worms.len(), 2);
    }

    #[test]
    fn pools_start_empty() {
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            1,
            &[0u8; 256],
            Vec::new(),
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
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            7,
            &[0u8; 256],
            Vec::new(),
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
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            7,
            &[0u8; 256],
            Vec::new(),
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
    fn from_init_sets_control_defaults() {
        // The 9 Slice-3 control fields take their post-`ResetWorms`/ctor
        // constants (worm.hpp + game.cpp:164 ResetWorms). Verified per design
        // doc *Datamodel additions*.
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
        assert_eq!(w.aiming_speed, 0, "aiming_speed{{0}}");
        assert_eq!(w.direction, 0, "direction{{0}}");
        assert!(w.movable, "ctor sets movable(true)");
        assert!(!w.able_to_jump, "able_to_jump{{false}}");
        assert!(!w.able_to_dig, "able_to_dig{{false}}");
        assert!(!w.key_change_pressed, "key_change_pressed{{false}}");
        assert_eq!(w.current_weapon, 0, "ResetWorms sets current_weapon = 0");
        assert_eq!(w.fire_cone, 0, "fire_cone{{0}}");
        assert_eq!(w.leave_shell_timer, 0, "leave_shell_timer{{0}}");
    }

    #[test]
    fn control_state_press_sets_and_release_clears() {
        // press(n) sets the bit (C++ Press -> Set(n, true)); release(n) clears it
        // (C++ Release -> Set(n, false)). pack() reflects each change.
        let mut cs = ControlState::new();
        cs.press(ControlState::JUMP);
        assert!(cs.get(ControlState::JUMP));
        assert_eq!(cs.pack(), 1 << 6);
        // Pressing an already-set bit is idempotent.
        cs.press(ControlState::JUMP);
        assert_eq!(cs.pack(), 1 << 6);
        cs.release(ControlState::JUMP);
        assert!(!cs.get(ControlState::JUMP));
        assert_eq!(cs.pack(), 0);
        // Releasing a clear bit is a no-op.
        cs.release(ControlState::JUMP);
        assert_eq!(cs.pack(), 0);
    }

    #[test]
    fn control_state_pressed_once_returns_prior_bit_and_clears_it() {
        // PressedOnce (worm.hpp:191-195): read the bit, clear it, return the
        // prior value. The clear must be visible in pack().
        let mut cs = ControlState::new();
        cs.press(ControlState::LEFT);
        cs.press(ControlState::RIGHT);
        // First read of LEFT: true, and the bit is consumed.
        assert!(cs.pressed_once(ControlState::LEFT), "set bit -> true");
        assert!(!cs.get(ControlState::LEFT), "pressed_once cleared LEFT");
        assert_eq!(cs.pack(), 1 << 3, "only RIGHT remains set");
        // Second read of LEFT: now false (already cleared), still clear.
        assert!(!cs.pressed_once(ControlState::LEFT), "clear bit -> false");
        assert_eq!(cs.pack(), 1 << 3, "RIGHT still set, LEFT stays clear");
        // RIGHT is independent and still consumable.
        assert!(
            cs.pressed_once(ControlState::RIGHT),
            "RIGHT still set -> true"
        );
        assert_eq!(cs.pack(), 0, "both bits now cleared");
    }

    #[test]
    fn resolve_weapons_mirrors_init_weapons() {
        // Build a synthetic Objects table whose ammo == 10 * index, then resolve
        // through a non-identity weap_order to prove the indirection is applied.
        use assets::object::{Objects, Weapon};
        let weapons: Vec<Weapon> = (0..5)
            .map(|i| Weapon {
                id: i,
                ammo: i * 10,
                ..Default::default()
            })
            .collect();
        let objects = Objects {
            weapons,
            ..Default::default()
        };
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
        assert!(
            !lvl.checked_mat_background(0, 1000),
            "large y is OOB -> entry 0"
        );
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
        let state = SimState::new(
            &level,
            &two_worms(),
            0,
            &flags,
            Vec::new(),
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
        assert_eq!(
            state.level.material_flags, flags,
            "flag table copied verbatim"
        );
        // synthetic_level idx 2 (x=2,y=0) = material 7 -> background.
        assert!(state.level.checked_mat_background(2, 0));
        // idx 0 = material 1 -> no flag set -> false.
        assert!(!state.level.checked_mat_background(0, 0));
    }

    // ---- Slice 4a datamodel scaffolding --------------------------------------

    #[test]
    fn wobject_owner_idx_defaults_zero() {
        // The new field defaults to 0 (C++ owner_idx for the unused/default case).
        assert_eq!(WObject::default().owner_idx, 0);
    }

    #[test]
    fn wobject_owner_idx_is_not_hashed() {
        // owner_idx must NOT fold into the master hash (it is omitted from the
        // wobject fold in hash.rs / C++ stateHash), so two states differing only
        // in owner_idx hash identically. This keeps slices 1-3 goldens green.
        use crate::hash::hash_game_state;
        let level = synthetic_level();
        let mk = |owner: i32| {
            let mut s = SimState::new(
                &level,
                &two_worms(),
                0,
                &[0u8; 256],
                Vec::new(),
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
            s.wobjects.spawn(WObject {
                pos: Vec2::new(1, 2),
                vel: Vec2::new(3, 4),
                cur_frame: 5,
                time_left: 6,
                ty: Some(1),
                owner_idx: owner,
            });
            hash_game_state(&s)
        };
        assert_eq!(mk(0), mk(9), "owner_idx must not affect the master hash");
    }

    #[test]
    fn worm_weapon_available_is_loading_left_zero() {
        // Available() == (loading_left == 0), independent of ammo/delay_left.
        let mut ww = WormWeapon::default();
        assert!(ww.available(), "loading_left 0 -> available");
        ww.loading_left = 1;
        assert!(!ww.available(), "loading_left > 0 -> not available");
        ww.loading_left = -3; // any non-zero value is "still loading"
        assert!(!ww.available(), "loading_left != 0 -> not available");
        // available() ignores ammo and delay_left (the gate tests those itself).
        ww.loading_left = 0;
        ww.ammo = 0;
        ww.delay_left = 99;
        assert!(ww.available(), "available() ignores ammo and delay_left");
    }

    #[test]
    fn inside_is_a_true_range_check_distinct_from_the_wrap() {
        let lvl = probe_level(); // 4x4
        assert!(lvl.inside(0, 0));
        assert!(lvl.inside(3, 3));
        assert!(!lvl.inside(4, 0), "x == width is outside");
        assert!(!lvl.inside(0, 4), "y == height is outside");
        assert!(!lvl.inside(-1, 0), "negative x is outside");
        assert!(!lvl.inside(0, -1), "negative y is outside");
        // The trap: (-1,1) flattens to in-range idx 3 for the WRAPPING probe, but
        // inside() rejects it on the real range check — proving they differ.
        assert!(
            lvl.checked_mat_background(-1, 1),
            "wrapping probe reads idx 3 (in range)"
        );
        assert!(
            !lvl.inside(-1, 1),
            "inside() is a real range check, NOT the wrap"
        );
    }

    // A 4x4 level pinning every DirtRock branch: a dirt cell, a dirt2 cell, a
    // rock cell, a background-only cell, a no-flag cell. material_id[3] is dirt so
    // a *wrapping* dirt_rock(-1,1) would read it (idx 3) and wrongly report true —
    // the inside() gate must reject it instead.
    fn dirt_rock_level() -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[1] = MAT_DIRT; // dirt -> DirtRock
        material_flags[2] = MAT_ROCK; // rock -> DirtRock
        material_flags[3] = MAT_DIRT2; // dirt2 -> DirtRock
        material_flags[4] = MAT_BACKGROUND; // background only -> NOT DirtRock

        let mut material_id = vec![0u8; 16]; // material 0: no flags
        material_id[0] = 1; // (0,0) dirt
        material_id[3] = 1; // (3,0) dirt -> the wrapped cell for (-1,1)
        material_id[5] = 2; // (1,1) rock
        material_id[6] = 3; // (2,1) dirt2
        material_id[10] = 4; // (2,2) background only

        LevelSim {
            width: 4,
            height: 4,
            material_id,
            material_flags,
        }
    }

    #[test]
    fn dirt_rock_tests_the_dirt_dirt2_rock_bits() {
        let lvl = dirt_rock_level();
        assert!(lvl.dirt_rock(0, 0), "dirt cell -> DirtRock");
        assert!(lvl.dirt_rock(1, 1), "rock cell -> DirtRock");
        assert!(lvl.dirt_rock(2, 1), "dirt2 cell -> DirtRock");
        assert!(
            !lvl.dirt_rock(2, 2),
            "background-only cell -> NOT DirtRock (bit 3 excluded)"
        );
        assert!(!lvl.dirt_rock(3, 3), "no-flag material -> NOT DirtRock");
        // OOB -> false (NOT the wrapping fallback): inside() gates first.
        assert!(!lvl.dirt_rock(100, 100), "far OOB -> false");
        assert!(
            !lvl.dirt_rock(-1, 1),
            "negative x is OOB for dirt_rock: inside() gate beats the wrap (idx 3 is dirt)"
        );
    }

    // ---- Slice 4b datamodel: flag-read predicates + set_material -------------

    // A 4x4 level whose top row pins one cell of each material class:
    // (0,0) background, (1,0) dirt, (2,0) dirt2, (3,0) rock. material 0 (the
    // default fill) carries NO flags, so every other cell is "nothing".
    fn flag_read_level() -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[1] = MAT_BACKGROUND;
        material_flags[2] = MAT_DIRT;
        material_flags[3] = MAT_DIRT2;
        material_flags[4] = MAT_ROCK;

        let mut material_id = vec![0u8; 16]; // material 0: no flags
        material_id[0] = 1; // (0,0) background
        material_id[1] = 2; // (1,0) dirt
        material_id[2] = 3; // (2,0) dirt2
        material_id[3] = 4; // (3,0) rock

        LevelSim {
            width: 4,
            height: 4,
            material_id,
            material_flags,
        }
    }

    #[test]
    fn flag_reads_discriminate_background_dirt_dirt2_rock() {
        let lvl = flag_read_level();
        // background cell: only `background` is true.
        assert!(lvl.background(0, 0), "background cell -> background");
        assert!(!lvl.any_dirt(0, 0));
        assert!(!lvl.dirt(0, 0));
        assert!(!lvl.dirt2(0, 0));
        // dirt cell: `any_dirt` + `dirt` true, `dirt2`/`background` false.
        assert!(!lvl.background(1, 0));
        assert!(lvl.any_dirt(1, 0), "dirt cell -> any_dirt");
        assert!(lvl.dirt(1, 0), "dirt cell -> dirt");
        assert!(!lvl.dirt2(1, 0));
        // dirt2 cell: `any_dirt` + `dirt2` true, `dirt`/`background` false.
        assert!(!lvl.background(2, 0));
        assert!(lvl.any_dirt(2, 0), "dirt2 cell -> any_dirt");
        assert!(!lvl.dirt(2, 0));
        assert!(lvl.dirt2(2, 0), "dirt2 cell -> dirt2");
        // rock cell: none of the four background/dirt predicates fire.
        assert!(!lvl.background(3, 0));
        assert!(!lvl.any_dirt(3, 0), "rock is not dirt");
        assert!(!lvl.dirt(3, 0));
        assert!(!lvl.dirt2(3, 0));
    }

    #[test]
    fn set_material_updates_the_map_and_only_that_cell() {
        let mut lvl = flag_read_level();
        // (1,1) = idx 5 starts as material 0 (no flags): nothing reads true.
        assert!(!lvl.background(1, 1));
        assert!(!lvl.dirt(1, 1));
        // Point it at material 1 (background): the background read flips true.
        lvl.set_material(5, 1);
        assert!(
            lvl.background(1, 1),
            "set_material 1 -> background reads true"
        );
        assert!(!lvl.dirt(1, 1));
        // Re-point at material 2 (dirt): the dirt read flips true, background off.
        lvl.set_material(5, 2);
        assert!(lvl.dirt(1, 1), "set_material 2 -> dirt reads true");
        assert!(!lvl.background(1, 1));
        // set_material touches ONLY idx 5 — the (0,0) background cell is intact.
        assert!(lvl.background(0, 0), "neighbouring cell untouched");
    }

    #[test]
    fn sim_state_carries_large_sprites_and_textures() {
        // textures[6] is greenball: { mframe:38, rframe:2, sframe:82, ndrawback:false }.
        let mut textures = vec![Texture::default(); 7];
        textures[6] = Texture {
            mframe: 38,
            rframe: 2,
            sframe: 82,
            ndrawback: false,
        };
        // The large-sprite bank is 16x16 x 110 (C++ large_sprites.Allocate(16,16,110)).
        let large_sprites = SpriteSet {
            width: 16,
            height: 16,
            count: 110,
            data: vec![0u8; 110 * 16 * 16],
        };
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            0,
            &[0u8; 256],
            Vec::new(),
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            large_sprites,
            textures,
            Vec::new(),
            Vec::new(),
            100,
            true,
            100,
        );
        let t = &state.textures[6];
        assert_eq!(t.mframe, 38, "greenball mframe");
        assert_eq!(t.rframe, 2, "greenball rframe");
        assert_eq!(t.sframe, 82, "greenball sframe");
        assert!(!t.ndrawback, "greenball ndrawback");
        assert_eq!(
            state.large_sprites.sprite(38).len(),
            256,
            "a 16x16 large sprite is 256 bytes"
        );
    }

    #[test]
    fn sim_state_carries_cossin_and_weapons() {
        // cossin matches the sim-core table verbatim; weapons are carried as given.
        let weapons = vec![
            Weapon {
                id: 0,
                name: "A".into(),
                ammo: 5,
                ..Default::default()
            },
            Weapon {
                id: 1,
                name: "B".into(),
                ammo: 9,
                ..Default::default()
            },
        ];
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            0,
            &[0u8; 256],
            weapons.clone(),
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
        assert_eq!(
            state.cossin,
            sim_core::tables::precompute_cossin(),
            "cossin matches the sim-core table"
        );
        assert_eq!(state.weapons, weapons, "weapons carried verbatim");
        // Spot-check a known identity: index 0 holds sin(0) = 0 in x.
        assert_eq!(state.cossin[0].x, 0);
    }

    // ---- Slice 4c datamodel scaffolding --------------------------------------

    #[test]
    fn sobject_carries_position_and_anim_fields_and_is_copy() {
        // SObject gains x/y/anim_delay (C++ sobject.hpp:89-95); all default 0.
        let s = SObject::default();
        assert_eq!(s.id, 0);
        assert_eq!(s.x, 0);
        assert_eq!(s.y, 0);
        assert_eq!(s.cur_frame, 0);
        assert_eq!(s.anim_delay, 0);
        // Copy: assigning leaves the source usable.
        let s2 = SObject {
            id: 2,
            x: -3,
            y: 4,
            cur_frame: 5,
            anim_delay: 6,
        };
        let s3 = s2; // Copy, not move
        assert_eq!(s2, s3);
    }

    #[test]
    fn pool_of_sobject_spawns_lowest_free_and_iterates_slot_order() {
        // The generic Pool<T> works for SObject: cap 700, lowest-free spawn,
        // slot-order iteration (mirrors the wobjects pool contract).
        let mut pool: Pool<SObject> = Pool::new(SOBJECT_CAPACITY);
        assert_eq!(pool.capacity(), 700);
        assert!(pool.is_empty());
        let mk = |id: i32| SObject {
            id,
            x: id * 10,
            y: id * 20,
            cur_frame: 0,
            anim_delay: 0,
        };
        assert_eq!(pool.spawn(mk(1)), Some(0));
        assert_eq!(pool.spawn(mk(2)), Some(1));
        assert_eq!(pool.spawn(mk(3)), Some(2));
        // Free the middle slot; the next spawn reuses the lowest free index.
        pool.free(1);
        assert_eq!(pool.len(), 2);
        let ids: Vec<i32> = pool.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 3], "iter yields live slots in index order");
        assert_eq!(pool.spawn(mk(4)), Some(1), "reuses lowest free slot");
        let ids: Vec<i32> = pool.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 4, 3]);
    }

    #[test]
    fn nobject_owner_idx_and_time_left_default_zero() {
        // The new fields default to 0 (C++ NObject owner_idx/time_left).
        let n = NObject::default();
        assert_eq!(n.owner_idx, 0);
        assert_eq!(n.time_left, 0);
    }

    #[test]
    fn sim_state_carries_object_type_tables() {
        // SimState carries the sobject/nobject parameter tables verbatim. Use the
        // shapes Slice-4c reads: sobject_types[2] = small_explosion, nobject_types[2]
        // = particle__disappearing (their TC field values are pinned against the real
        // data in the oracle test; here we only prove SimState::new threads them).
        let mut sobject_types = vec![SObjectType::default(); 3];
        sobject_types[2] = SObjectType {
            num_sounds: 2,
            anim_delay: 2,
            num_frames: 5,
            detect_range: 8,
            damage: 5,
            blow_away: 3000,
            dirt_effect: 2,
            id: 2,
            id_str: "small_explosion".into(),
            ..Default::default()
        };
        let mut nobject_types = vec![NObjectType::default(); 3];
        nobject_types[2] = NObjectType {
            speed: 80,
            speed_v: 40,
            distribution: 10000,
            gravity: 700,
            expl_ground: true,
            id: 2,
            id_str: "particle__disappearing".into(),
            ..Default::default()
        };
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            0,
            &[0u8; 256],
            Vec::new(),
            PhysicsConsts::default(),
            ControlConsts::default(),
            false,
            SpriteSet::default(),
            Vec::new(),
            sobject_types.clone(),
            nobject_types.clone(),
            100,
            true,
            100,
        );
        assert_eq!(
            state.sobject_types, sobject_types,
            "sobject table carried verbatim"
        );
        assert_eq!(
            state.nobject_types, nobject_types,
            "nobject table carried verbatim"
        );
        // Spot-check the Slice-4c shapes survive the round-trip.
        assert_eq!(state.sobject_types[2].id_str, "small_explosion");
        assert_eq!(state.sobject_types[2].damage, 5);
        assert_eq!(state.nobject_types[2].id_str, "particle__disappearing");
        assert!(state.nobject_types[2].expl_ground);
    }

    #[test]
    fn settings_loading_time_and_load_change_defaults() {
        // C++ defaults: loading_time = 100 (settings.hpp:75), load_change = true
        // (settings.hpp:79). These are settings scalars; they are NOT hashed.
        let state = SimState::new(
            &synthetic_level(),
            &two_worms(),
            0,
            &[0u8; 256],
            Vec::new(),
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
        assert_eq!(
            state.settings_loading_time, 100,
            "C++ default loading_time = 100 (settings.hpp:75)"
        );
        assert!(state.load_change, "C++ default load_change = true (settings.hpp:79)");
    }

    // Build an idle tick-0 state (two invisible worms, empty pools): nothing inside
    // process_frame draws rand EXCEPT the bonus-drop roll, so rand.last() isolates it.
    fn idle_state(seed: u32) -> SimState {
        SimState::new(
            &synthetic_level(),
            &two_worms(),
            seed,
            &[0u8; 256],
            Vec::new(),
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
        )
    }

    #[test]
    fn bonus_drop_roll_defaults_off_and_draws_no_rand() {
        // `settings_max_bonuses` defaults to 0 (the in-game default is 4, but the
        // dumper/difftest set it explicitly; priors leave it 0). With it 0 the
        // `max_bonuses > 0 && rand(...)` gate SHORT-CIRCUITS before the rand draw — so
        // the per-tick roll consumes NO randomness. With two invisible worms and empty
        // pools, nothing else in process_frame draws either, so rand.last() must stay
        // pinned at the post-seed 0 across the whole tick. THIS is why slices 1-5b stay
        // byte-identical: no rand drawn => no perturbation.
        let mut state = idle_state(42);
        assert_eq!(state.settings_max_bonuses, 0, "max_bonuses defaults to 0");
        // Set the chance NONZERO to prove it is the `max_bonuses == 0` gate, not a zero
        // chance, that suppresses the draw (sharp non-tautology).
        state.bonus_drop_chance = 11;
        assert_eq!(state.rand.last(), 0, "no RNG consumed at tick 0");

        state.process_frame(&[]);

        assert_eq!(
            state.rand.last(),
            0,
            "max_bonuses == 0 short-circuits: the bonus-drop roll draws NO rand"
        );
        assert_eq!(state.cycles, 1, "cycles still advances once per tick");
    }

    #[test]
    fn bonus_drop_roll_draws_once_when_max_bonuses_positive() {
        // With max_bonuses > 0 the gate opens and the roll draws `rand(CBonusDropChance)`
        // EXACTLY once per tick. A reference Rand seeded identically, advanced by ONE
        // bound() call, must match the driven state's rand — proving the single draw at
        // the load-bearing position (after ++cycles, before the worm loop).
        let mut state = idle_state(42);
        state.settings_max_bonuses = 4;
        state.bonus_drop_chance = 11;

        let mut reference = Rand::new();
        reference.seed(42);
        let bounded = reference.bound(11);
        // The roll runs create_bonus iff the bounded draw is 0. This test isolates the
        // SINGLE bonus-drop roll draw, so guard the seed/chance to keep it NON-zero —
        // otherwise create_bonus would fire and draw its own placement cluster, and
        // `rand.last()` would no longer equal the reference's single draw. (create_bonus
        // itself is covered by the bonus.rs unit tests.) If a future RNG change makes
        // this 0, the assert fails loudly.
        assert_ne!(
            bounded, 0,
            "test seed/chance must keep the roll != 0 so it isolates the single draw"
        );

        state.process_frame(&[]);

        // Exactly one next_u32 was consumed, and it was the bonus-drop roll's
        // rand.bound(bonus_drop_chance): last() matches the reference's single draw.
        assert_eq!(
            state.rand.last(),
            reference.last(),
            "the bonus-drop roll draws rand(CBonusDropChance) exactly once per tick"
        );
        assert_ne!(
            state.rand.last(),
            0,
            "a draw genuinely occurred (rand advanced off the post-seed 0)"
        );
        assert_eq!(state.cycles, 1, "cycles advances once per tick");
    }

    // ----- Slice 5d T1: clamp + lives gate + visible/dead arm split ---------
    // `idle_state` builds TWO INVISIBLE worms (`two_worms` sets `visible: false`,
    // `killed_timer: 150`, `lives: 5`, `health: 100`) via `SimState::new`
    // (`settings_health` defaults to 100). The dead-worm `else` arm's
    // `killed_timer` countdown is the cleanest non-hashed witness for the gate/
    // split without needing a physics/gravity setup; all cases keep
    // `killed_timer` at 150→149 so the `begin_respawn`/`do_respawning` stubs
    // (reached only at `== 0` / `< 0`) are never hit.

    #[test]
    fn t1_health_clamp_runs_every_tick_outside_the_lives_gate() {
        // worm.cpp:213 `health = min(health, settings->health)` runs BEFORE the
        // lives gate (:215) — so it clamps even a `lives == 0` worm whose body is
        // skipped. This pins the clamp OUTSIDE the gate (a stronger statement than
        // "clamp runs"). settings_health defaults to 100.
        let mut state = idle_state(1);
        // Gate-closed (lives==0), invisible worm with above-max health: ONLY the
        // clamp can touch it, and the skipped body must leave killed_timer frozen.
        state.worms[0].lives = 0;
        state.worms[0].health = 150;
        assert!(!state.worms[0].visible);
        // Full-health worm: the clamp is the identity.
        state.worms[1].health = 100;

        state.process_frame(&[]);

        assert_eq!(
            state.worms[0].health, 100,
            "clamp caps to settings_health even with the lives gate closed"
        );
        assert_eq!(
            state.worms[0].killed_timer, 150,
            "gate closed (lives==0) => dead arm skipped, killed_timer frozen"
        );
        assert_eq!(
            state.worms[1].health, 100,
            "clamp is the identity for a full-health worm"
        );
    }

    #[test]
    fn t1_lives_gate_skips_the_whole_worm_body() {
        // worm.cpp:215: in KillEmAll the whole `Worm::Process` body runs iff
        // `lives > 0`. Witness via the dead-arm killed_timer countdown (invisible
        // worm): it decrements with lives>0 and is frozen with lives==0. The
        // lives>0 branch fails on the pre-restructure base (no dead arm), so this
        // is non-vacuous.
        let mut state = idle_state(2);
        state.worms[0].lives = 5; // gate open
        state.worms[1].lives = 0; // gate closed
        assert!(!state.worms[0].visible && !state.worms[1].visible);
        assert_eq!(state.worms[0].killed_timer, 150);
        assert_eq!(state.worms[1].killed_timer, 150);

        state.process_frame(&[]);

        assert_eq!(
            state.worms[0].killed_timer, 149,
            "lives>0: the dead arm ran (killed_timer counted down)"
        );
        assert_eq!(
            state.worms[1].killed_timer, 150,
            "lives==0: the entire worm body was skipped (killed_timer frozen)"
        );
    }

    #[test]
    fn t1_dead_arm_pressed_once_fire_and_steerable_reset() {
        // worm.cpp:433-437 dead arm: `steerable_count = 0`; `PressedOnce(kFire)`
        // reads the Fire bit, CLEARS it (worm.hpp:187-191), and sets `ready` on a
        // hit. Non-tautological: start ready=false + steerable_count=7 and drive
        // Fire, so both the set and the clear are real edges.
        let mut state = idle_state(3);
        state.worms[0].lives = 5; // gate open, invisible => dead arm
        state.worms[0].ready = false; // start not-ready so the set is a real edge
        state.worms[0].steerable_count = 7; // must be zeroed each dead tick

        let mut fire = ControlState::new();
        fire.set(ControlState::FIRE, true);
        // worm0 gets Fire; worm1 gets empty input (its dead arm just counts down).
        state.process_frame(&[fire, ControlState::new()]);

        assert!(state.worms[0].ready, "PressedOnce(kFire) set ready");
        assert!(
            !state.worms[0].control_states.get(ControlState::FIRE),
            "PressedOnce(kFire) cleared the Fire bit (read-and-clear semantics)"
        );
        assert_eq!(
            state.worms[0].steerable_count, 0,
            "steerable_count zeroed each dead tick"
        );
        assert_eq!(
            state.worms[0].killed_timer, 149,
            "killed_timer counted down in the dead arm"
        );
    }

    // ----- Slice 5d T2: pre-death blood drip (worm.cpp:355-367) -------------
    // The drip fires at the END of the visible arm while a worm is alive but
    // under settings_health/4. RNG contract (verified against :355-367):
    //   rand(health+6)            [outer gate]
    //   on 0 -> rand(3)           [inner gate]
    //   on 0 -> rand(3)           [sound index 18 + rand(3)]
    //   then, within the OUTER gate (outside the sound gate):
    //     nobject_types[6].Create1 = rand(dist*2) x2   [the blood spawn]
    // Tests drive the standalone `worm_pre_death_drip` against a SEEDED Rand
    // and replay a reference stream to pin the exact draw ORDER + COUNT (so
    // an order swap / miscount / mis-nested gate is detectable — non-taut).

    fn drip_worm(health: i32) -> WormState {
        let mut w = WormState::from_init(&two_worms()[1]);
        w.health = health;
        w
    }

    // Blood is nobject_types[6]: distribution != 0 so Create1 draws two
    // rand(dist*2) (x, y); start_frame <= 0 & color == 0 & color_bullets == 0
    // => Create takes the color path (no extra draw). Exactly 2 draws / spawn.
    fn blood_types() -> Vec<NObjectType> {
        let mut v = vec![NObjectType::default(); 7];
        v[6] = NObjectType {
            distribution: 10000,
            start_frame: 0,
            color_bullets: 0,
            ..Default::default()
        };
        v
    }

    fn seeded_rand(s: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(s);
        r
    }

    // A seed whose SECOND draw (after the forced-0 `bound(1)` outer roll on a
    // health=-5 worm, `health+6 == 1`) has `bound(3) == 0` (inner gate open =>
    // sound draw) or `!= 0` (inner gate closed => no sound draw).
    fn seed_forcing_inner(inner_zero: bool) -> u32 {
        for s in 0u32..2_000_000 {
            let mut r = seeded_rand(s);
            let _outer = r.bound(1); // health+6 == 1 => bound(1) is always 0
            if (r.bound(3) == 0) == inner_zero {
                return s;
            }
        }
        panic!("no seed forces inner_zero = {inner_zero}");
    }

    // A seed whose FIRST `bound(m)` is non-zero (forces the outer gate CLOSED).
    fn seed_forcing_outer_nonzero(m: u32) -> u32 {
        for s in 0u32..2_000_000 {
            let mut r = seeded_rand(s);
            if r.bound(m) != 0 {
                return s;
            }
        }
        panic!("no seed forces a non-zero outer roll for m = {m}");
    }

    #[test]
    fn t2_drip_gate_closed_at_or_above_quarter_health_draws_nothing() {
        // health >= settings_health/4 (integer /): the whole drip is skipped —
        // no draw, no spawn. settings_health=100 => quarter = 25; health=25 is
        // NOT < 25 (the boundary), so the gate is closed. This pins the integer
        // `/4` and the `<` (not `<=`).
        let w = drip_worm(25);
        let types = blood_types();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(7);
        let before = rand.last();

        worm_pre_death_drip(&w, 1, 100, &types, &mut rand, &mut pool);

        assert_eq!(rand.last(), before, "closed gate draws NO rand");
        assert_eq!(pool.len(), 0, "closed gate spawns nothing");
    }

    #[test]
    fn t2_drip_nonzero_outer_draws_one_and_spawns_nothing() {
        // health < settings_health/4 opens the outer gate; a NON-ZERO outer
        // roll draws exactly ONE value (rand(health+6)) and spawns nothing (no
        // inner, no sound, no Create1).
        let health = 24; // 24 < 25; health+6 = 30
        let seed = seed_forcing_outer_nonzero(30);
        let w = drip_worm(health);
        let types = blood_types();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(seed);

        // Reference: exactly one bound(30), asserted non-zero (seed guard).
        let mut refr = seeded_rand(seed);
        let outer = refr.bound(30);
        assert_ne!(outer, 0, "seed guard: the outer roll must be non-zero");

        worm_pre_death_drip(&w, 1, 100, &types, &mut rand, &mut pool);

        assert_eq!(rand.last(), refr.last(), "exactly ONE outer draw");
        assert_eq!(pool.len(), 0, "non-zero outer => no blood spawn");
    }

    #[test]
    fn t2_drip_zero_outer_nonzero_inner_spawns_without_sound() {
        // Forced-0 outer (health=-5 => health+6 = 1 => bound(1) == 0 always)
        // and a NON-ZERO inner roll: draws outer + inner, SKIPS the sound draw,
        // then ALWAYS spawns blood (Create1 = 2 draws). This is the load-bearing
        // case: the Create1 spawn sits INSIDE the outer gate but OUTSIDE the
        // sound gate. Order: bound(1), bound(3), bound(20000), bound(20000).
        let seed = seed_forcing_inner(false);
        let w = drip_worm(-5);
        let types = blood_types();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(seed);

        let mut refr = seeded_rand(seed);
        let outer = refr.bound(1);
        assert_eq!(outer, 0, "health+6 == 1 => bound(1) is always 0 (forced outer)");
        let inner = refr.bound(3);
        assert_ne!(inner, 0, "seed guard: inner non-zero => no sound draw");
        let _dx = refr.bound(20000); // Create1 x
        let _dy = refr.bound(20000); // Create1 y

        worm_pre_death_drip(&w, 1, 100, &types, &mut rand, &mut pool);

        assert_eq!(
            rand.last(),
            refr.last(),
            "draws outer + inner + Create1(x,y); NO sound draw"
        );
        assert_eq!(
            pool.len(),
            1,
            "Create1 spawns inside the outer gate regardless of the inner/sound gate"
        );
    }

    #[test]
    fn t2_drip_zero_outer_zero_inner_draws_sound_then_spawns() {
        // Forced-0 outer AND forced-0 inner: adds the sound draw (18 + rand(3))
        // BEFORE Create1. Order: bound(1), bound(3), bound(3) [sound],
        // bound(20000), bound(20000). The sound draw sits inside the inner gate;
        // Create1 sits outside it (but inside the outer gate) => exactly ONE
        // more draw than the no-sound case, plus the spawn.
        let seed = seed_forcing_inner(true);
        let w = drip_worm(-5);
        let types = blood_types();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(seed);

        let mut refr = seeded_rand(seed);
        let outer = refr.bound(1);
        assert_eq!(outer, 0, "forced outer 0");
        let inner = refr.bound(3);
        assert_eq!(inner, 0, "seed guard: inner == 0 opens the sound gate");
        let _snd = 18 + refr.bound(3); // sound index draw (18 + rand(3))
        let _dx = refr.bound(20000); // Create1 x
        let _dy = refr.bound(20000); // Create1 y

        worm_pre_death_drip(&w, 1, 100, &types, &mut rand, &mut pool);

        assert_eq!(
            rand.last(),
            refr.last(),
            "draws outer + inner + SOUND + Create1(x,y)"
        );
        assert_eq!(pool.len(), 1, "blood spawns after the sound draw");
    }

    // ----- Slice 5d T3: death block (worm.cpp:369-426) ----------------------
    // The death block runs at the END of the visible arm when health <= 0. RNG
    // contract (verified against :369-426):
    //   rand(3)                          [death sound 15 + rand(3); the ONLY
    //                                      pre-spray draw]
    //   --lives / kill bookkeeping        [no rand]
    //   kMax = 120*blood/100; iff kMax>1: [strict >, NOT >=1]
    //     for i in 1..=kMax:  rand(128)   [angle arg, OUTSIDE Create2]
    //                         + nobject_types[6].Create2 (its own draws)
    //   for i in {7,21,..,105} (8 iters): rand(14)  [angle arg, OUTSIDE Create2]
    //                         + nobject_types[index].Create2  [PER-WORM gib type]
    // Tests drive the standalone `worm_death` against a SEEDED Rand and replay a
    // reference stream to pin the exact draw ORDER + COUNT + spawn count.

    // Synthetic nobject_types for the death block. Blood (index 6): speed_v +
    // distribution != 0 => Create2 draws 3 (speed, dist x, dist y); start_frame
    // <= 0 & time_to_explo_v == 0 => no Create draw. Per blood particle =
    // rand(128) + 3 = 4 draws. Worm-gib types (indices 0/1) are given DISTINCT
    // shapes so the per-worm indexing (`nobject_types[index]`, not [6]) is
    // observable: gib[0] has distribution == 0 => Create2 draws only 1 (speed);
    // gib[1] has distribution != 0 => Create2 draws 3. Per gib = rand(14) + the
    // type's own draws.
    fn death_types() -> Vec<NObjectType> {
        let mut v = vec![NObjectType::default(); 7];
        // Blood (type 6): 3 internal draws.
        v[6] = NObjectType {
            speed_v: 40,
            distribution: 10000,
            start_frame: 0,
            time_to_explo_v: 0,
            ..Default::default()
        };
        // Gib type for worm 0: distribution == 0 => ONLY the speed draw (1).
        v[0] = NObjectType {
            speed_v: 20,
            distribution: 0,
            start_frame: 0,
            time_to_explo_v: 0,
            ..Default::default()
        };
        // Gib type for worm 1: distribution != 0 => speed + dist x + dist y (3).
        v[1] = NObjectType {
            speed_v: 20,
            distribution: 5000,
            start_frame: 0,
            time_to_explo_v: 0,
            ..Default::default()
        };
        v
    }

    // A dying worm (health <= 0), visible, with a known last_killed_by_idx and a
    // known velocity (so vel/3 into Create2 is exercised).
    fn dying_worm(index: usize, health: i32, killed_by: i32) -> WormState {
        let mut w = WormState::from_init(&two_worms()[index]);
        w.health = health;
        w.visible = true;
        w.last_killed_by_idx = killed_by;
        w.vel = Vec2::new(300, -600);
        // Non-default sentinels the death block must overwrite/clear.
        w.fire_cone = 9;
        w.leave_shell_timer = 9;
        w.make_sight_green = true;
        w.ninjarope.out = true;
        w.control_states.set(ControlState::FIRE, true);
        w
    }

    #[test]
    fn t3_death_gate_health_positive_draws_nothing_and_no_mutation() {
        // health > 0: the death block is skipped entirely — no draw, no spawn, no
        // field mutation, no killer. Pins the `health <= 0` gate (:369).
        let mut w = dying_worm(1, 1, 0); // health 1 > 0
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(7);
        let before = rand.last();
        let (mut lki, mut gc) = (-1i32, false);

        let killer = worm_death(&mut w, 1, 100, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(rand.last(), before, "live worm: death block draws NO rand");
        assert_eq!(pool.len(), 0, "live worm: no spray");
        assert!(w.visible, "live worm stays visible");
        assert_eq!(w.killed_timer, 150, "killed_timer untouched (still initial)");
        assert_eq!(killer, None, "no death => no kill attribution");
    }

    #[test]
    fn t3_death_sound_lives_visible_timer_and_field_clears() {
        // health <= 0 with NO blood spray (blood 0 => kMax 0) isolates the single
        // rand(3) death sound + the 8-gib spray. Worm index 0 => gib type[0]
        // (distribution 0 => 1 internal draw / gib). Expected draws:
        //   rand(3)                       [1, death sound]
        //   8 x (rand(14) + speed)        [8 x 2 = 16]
        // => 17 draws total; 8 nobjects (gibs only). Asserts the state mutations.
        let mut w = dying_worm(0, 0, -1); // no killer
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(11);
        let (mut lki, mut gc) = (-1i32, false);

        // Reference stream: sound + 8 gibs each (angle + 1 speed draw).
        let mut refr = seeded_rand(11);
        let _snd = 15 + refr.bound(3);
        for _ in 0..8 {
            let _angle = refr.bound(14); // gib angle arg
            let _speed = refr.bound(20); // gib[0] speed_v (distribution 0 => no dist draws)
        }

        let killer = worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(rand.last(), refr.last(), "draws: sound(1) + 8 gibs x (angle+speed)");
        assert_eq!(pool.len(), 8, "blood 0 => kMax 0 => NO blood spray; exactly 8 gibs");
        assert!(!w.visible, "visible=false on death (:407)");
        assert_eq!(w.killed_timer, 150, "killed_timer = kKilledTimerInitial (:408)");
        assert_eq!(w.lives, 4, "--lives (KillEmAll, :390): started 5");
        assert_eq!(w.fire_cone, 0, "fire_cone = 0 (:381)");
        assert_eq!(w.leave_shell_timer, 0, "leave_shell_timer = 0 (:370)");
        assert!(!w.make_sight_green, "make_sight_green = false (:371)");
        assert!(!w.ninjarope.out, "ninjarope.out = false (:382)");
        assert!(
            !w.control_states.get(ControlState::FIRE),
            "Release(kFire) clears the Fire bit (:425) — folded into the master hash"
        );
        assert_eq!(killer, None, "last_killed_by_idx < 0 => no kills++");
        assert_eq!(lki, 0, "KillEmAll: game.last_killed_idx = index");
        assert!(gc, "got_changed = (old(-1) != 0)");
    }

    #[test]
    fn t3_kills_attributed_to_the_killer_when_a_different_worm() {
        // last_killed_by_idx >= 0 && != index => Some(killer). Worm index 1 killed
        // by worm 0 => Some(0). (kMax 0 to keep the draw stream irrelevant here.)
        let mut w = dying_worm(1, 0, 0);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(3);
        let (mut lki, mut gc) = (-1i32, false);

        let killer = worm_death(&mut w, 1, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(killer, Some(0), "killer worm 0 gets kills++ (:403-405)");
    }

    #[test]
    fn t3_kills_not_attributed_on_self_kill_or_no_killer() {
        // last_killed_by_idx == index (self) => None; last_killed_by_idx < 0
        // (unknown) => None. Pins BOTH halves of the `>= 0 && != index` predicate.
        let types = death_types();
        let cossin = precompute_cossin();

        let mut w_self = dying_worm(1, 0, 1); // killed_by == index 1 (self)
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(3);
        let (mut lki, mut gc) = (-1i32, false);
        let self_killer =
            worm_death(&mut w_self, 1, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);
        assert_eq!(self_killer, None, "self-kill (killed_by == index) => no kills++");

        let mut w_none = dying_worm(0, 0, -1);
        let mut pool2: Pool<NObject> = Pool::new(600);
        let mut rand2 = seeded_rand(3);
        let (mut lki2, mut gc2) = (-1i32, false);
        let none_killer =
            worm_death(&mut w_none, 0, 0, &types, &cossin, &mut rand2, &mut pool2, &mut lki2, &mut gc2);
        assert_eq!(none_killer, None, "unknown killer (< 0) => no kills++");
    }

    #[test]
    fn t3_blood100_sprays_120_particles_480_draws_plus_8_gibs() {
        // blood = 100 => kMax = 120 (> 1) => 120 blood particles, each
        // rand(128) + Create2(blood: speed + dist x + dist y = 3) = 4 draws =>
        // 480 blood draws. Worm index 1 => gib type[1] (distribution != 0 => 3
        // internal): 8 gibs each rand(14) + 3 = 4 => 32 gib draws. Plus the
        // rand(3) sound => 1. Total = 1 + 480 + 32 = 513 draws; 128 nobjects.
        let mut w = dying_worm(1, -50, -1);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(99);
        let (mut lki, mut gc) = (-1i32, false);

        let mut refr = seeded_rand(99);
        let _snd = 15 + refr.bound(3); // sound
        // 120 blood particles: angle(128) + speed(40) + dist x(20000) + dist y(20000).
        for _ in 0..120 {
            refr.bound(128);
            refr.bound(40);
            refr.bound(20000);
            refr.bound(20000);
        }
        // 8 gibs (worm 1 => type[1], distribution 5000): angle(14) + speed(20)
        // + dist x(10000) + dist y(10000).
        for _ in 0..8 {
            refr.bound(14);
            refr.bound(20);
            refr.bound(10000);
            refr.bound(10000);
        }

        worm_death(&mut w, 1, 100, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(
            rand.last(),
            refr.last(),
            "1 (sound) + 120x4 (blood) + 8x4 (gib[1]) = 513 draws in order"
        );
        assert_eq!(pool.len(), 128, "120 blood + 8 gibs");
    }

    #[test]
    fn t3_blood1_gate_closed_no_spray_but_8_gibs_still_fire() {
        // blood = 1 => kMax = 120*1/100 = 1, which is NOT > 1 => the blood spray
        // is SKIPPED (pins the strict `> 1` gate, not `>= 1`). The 8-gib spray is
        // OUTSIDE that gate and still fires. Worm index 0 => gib type[0]
        // (distribution 0 => 1 internal). Draws: sound(1) + 8 x (angle + speed) =
        // 17; nobjects: 8 (gibs only). If the gate were `>= 1`, one blood
        // particle would spawn (9 nobjects) — this asserts it does NOT.
        let mut w = dying_worm(0, 0, -1);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(5);
        let (mut lki, mut gc) = (-1i32, false);

        let mut refr = seeded_rand(5);
        let _snd = 15 + refr.bound(3);
        for _ in 0..8 {
            refr.bound(14);
            refr.bound(20);
        }

        worm_death(&mut w, 0, 1, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(rand.last(), refr.last(), "blood 1 => no blood draws; only sound + 8 gibs");
        assert_eq!(pool.len(), 8, "kMax == 1 is NOT > 1 => no blood spray; exactly 8 gibs");
    }

    #[test]
    fn t3_gib_loop_runs_exactly_8_times_pinning_the_i_le_105_bound() {
        // The gib loop is `for i in (7..=105).step_by(14)` = {7,21,35,49,63,77,
        // 91,105} = 8 iterations (the overview's "7x" is wrong). With blood 0
        // (no blood spray) the pool holds EXACTLY the gibs, so pool.len() is the
        // iteration count witness: 8, not 7 (would miss i=105) and not 9.
        let mut w = dying_worm(0, 0, -1);
        let types = death_types();
        let cossin = precompute_cossin();
        let mut pool: Pool<NObject> = Pool::new(600);
        let mut rand = seeded_rand(1);
        let (mut lki, mut gc) = (-1i32, false);

        worm_death(&mut w, 0, 0, &types, &cossin, &mut rand, &mut pool, &mut lki, &mut gc);

        assert_eq!(pool.len(), 8, "gib loop runs EXACTLY 8 times ({{7,21,..,105}})");
    }

    #[test]
    fn t3_gib_spray_uses_the_per_worm_type_not_blood() {
        // The gib type is `nobject_types[index]` (per-worm), NOT blood[6]. gib[0]
        // (worm 0) has distribution 0 => 1 internal draw / gib; gib[1] (worm 1)
        // has distribution != 0 => 3 internal draws / gib. With blood 0 (no blood
        // spray) the ONLY difference between the two worms' draw counts is the gib
        // type: worm 0 => 1 + 8x(1+1) = 17; worm 1 => 1 + 8x(1+3) = 33. If the code
        // used blood[6] for gibs, both would draw the same. This discriminates.
        let types = death_types();
        let cossin = precompute_cossin();

        let mut w0 = dying_worm(0, 0, -1);
        let mut pool0: Pool<NObject> = Pool::new(600);
        let mut r0 = seeded_rand(2);
        let (mut lki0, mut gc0) = (-1i32, false);
        worm_death(&mut w0, 0, 0, &types, &cossin, &mut r0, &mut pool0, &mut lki0, &mut gc0);
        let mut ref0 = seeded_rand(2);
        ref0.bound(3); // sound
        for _ in 0..8 {
            ref0.bound(14);
            ref0.bound(20);
        }
        assert_eq!(r0.last(), ref0.last(), "worm 0 uses gib[0] (1 internal draw / gib)");

        let mut w1 = dying_worm(1, 0, -1);
        let mut pool1: Pool<NObject> = Pool::new(600);
        let mut r1 = seeded_rand(2);
        let (mut lki1, mut gc1) = (-1i32, false);
        worm_death(&mut w1, 1, 0, &types, &cossin, &mut r1, &mut pool1, &mut lki1, &mut gc1);
        let mut ref1 = seeded_rand(2);
        ref1.bound(3); // sound
        for _ in 0..8 {
            ref1.bound(14);
            ref1.bound(20);
            ref1.bound(10000);
            ref1.bound(10000);
        }
        assert_eq!(r1.last(), ref1.last(), "worm 1 uses gib[1] (3 internal draws / gib)");
        assert_ne!(
            r0.last(),
            r1.last(),
            "per-worm gib types => the two worms draw DIFFERENT amounts (not blood[6] for both)"
        );
    }

    // ------------------------------------------------------------------------
    // Slice 5d T4: BeginRespawn (worm.cpp:711-742) + CheckRespawnPosition
    // (game.cpp:611-650) — the level+enemy-dependent respawn-position search,
    // the canonical Step-2 desync trap. The RNG contract is 2 draws / trial
    // (rand(W) THEN rand(H)); the trial count = f(level pixels, live enemy pos).
    // ------------------------------------------------------------------------

    // Test spawn-rect / min-dist consts. W != H so the per-trial draw ORDER
    // (W FIRST, then H) is observable from the resulting candidate pos.
    const TSPAWN_X: i32 = 100;
    const TSPAWN_Y: i32 = 100;
    const TSPAWN_W: i32 = 100;
    const TSPAWN_H: i32 = 80;
    const TMIN_LAST: i32 = 30;
    const TMIN_ENEMY: i32 = 30;

    // A flat level: material 0 everywhere with an all-zero flag table, so both
    // `background()` (no drop-down) and `rock()` (empty Rock box) are false for
    // every pixel. Tests paint individual cells as needed.
    fn flat_level(width: i32, height: i32) -> LevelSim {
        LevelSim {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            material_flags: [0u8; 256],
        }
    }

    // A dead worm (visible=false) whose death position is the integer pixel
    // `(px, py)`. `logic_respawn` is seeded with a sentinel begin_respawn must
    // overwrite.
    fn dead_worm_at(index: usize, px: i32, py: i32) -> WormState {
        let mut w = WormState::from_init(&two_worms()[index]);
        w.visible = false;
        w.pos = Vec2::new(itof(px), itof(py));
        w.logic_respawn = Vec2::new(-999, -999);
        w.killed_timer = 0;
        w
    }

    // Run begin_respawn on `worms[index]` with the shared test consts.
    fn run_begin_respawn(worms: &mut [WormState], index: usize, level: &LevelSim, rand: &mut Rand) {
        begin_respawn(
            worms, index, level, TSPAWN_X, TSPAWN_Y, TSPAWN_W, TSPAWN_H, TMIN_LAST, TMIN_ENEMY, rand,
        );
    }

    #[test]
    fn t4_begin_respawn_one_trial_two_draws_w_then_h_and_sets_logic_respawn() {
        // Open ground, enemy + last-pos both far: trial 1 is accepted immediately.
        // Death pos (1000,1500) is large so the last-pos reject (kDeltaX = old_x,
        // the C++ bug) stays FALSE (|1000| > TMIN_LAST); enemy (3000,3000) is far.
        let level = flat_level(400, 400);
        let mut worms = vec![dead_worm_at(0, 1000, 1500), dead_worm_at(1, 3000, 3000)];

        // Reference stream: rand(W) FIRST, then rand(H). W != H pins the order.
        let mut refr = seeded_rand(1234);
        let cand_x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let cand_y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;

        let mut rand = seeded_rand(1234);
        run_begin_respawn(&mut worms, 0, &level, &mut rand);

        // logic_respawn = Ftoi(death) - (80,80).
        assert_eq!(
            worms[0].logic_respawn,
            Vec2::new(1000 - 80, 1500 - 80),
            "logic_respawn = death - (80,80)"
        );
        // EXACTLY 2 draws, in W-then-H order (RNG state matches the reference
        // after bound(W) then bound(H)); a swapped order or a retry would diverge.
        assert_eq!(rand.last(), refr.last(), "1-trial success => exactly 2 draws: rand(W) then rand(H)");
        // pos = candidate 1 (accepted, no drop-down on flat ground).
        assert_eq!(
            worms[0].pos,
            Vec2::new(itof(cand_x), itof(cand_y)),
            "pos.x = Itof(X + rand(W)); pos.y = Itof(Y + rand(H))"
        );
        assert_eq!(worms[0].killed_timer, -1, "killed_timer = -1 on exit (:741)");
    }

    #[test]
    fn t4_begin_respawn_enemy_too_close_forces_second_trial_four_draws() {
        // Placing the LIVE enemy worm ON trial-1's candidate makes the enemy
        // reject fire => a 2nd trial => 4 draws. Proves the enemy pos is read from
        // `worms[index ^ 1]` (not a constant).
        let level = flat_level(400, 400);
        let mut refr = seeded_rand(77);
        let c1x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c1y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        let c2x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c2y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        // `min_enemy = 0` => the enemy reject fires ONLY on the exact candidate
        // (|dx| <= 0 && |dy| <= 0), so trial-1 (enemy sits on it) rejects while
        // trial-2 clears as long as it is not pixel-identical to trial-1.
        assert!((c1x, c1y) != (c2x, c2y), "fixture: trial-2 differs from trial-1");

        // Death pos large => last-pos reject never fires. Enemy on candidate 1.
        let mut worms = vec![dead_worm_at(0, 1000, 1500), dead_worm_at(1, c1x, c1y)];
        let mut rand = seeded_rand(77);
        begin_respawn(
            &mut worms, 0, &level, TSPAWN_X, TSPAWN_Y, TSPAWN_W, TSPAWN_H, TMIN_LAST, 0, &mut rand,
        );

        assert_eq!(rand.last(), refr.last(), "2 trials => 4 draws (rand(W),rand(H) x2)");
        assert_eq!(
            worms[0].pos,
            Vec2::new(itof(c2x), itof(c2y)),
            "trial-1 rejected by the LIVE enemy pos; trial-2 accepted"
        );
        assert_eq!(worms[0].killed_timer, -1);
    }

    #[test]
    fn t4_begin_respawn_rock_in_box_forces_second_trial() {
        // A Rock() pixel inside trial-1's [x-3,x+3)x[y-4,y+4) box rejects it =>
        // a 2nd trial. Proves the CheckRespawnPosition rock scan feeds the count.
        let mut level = flat_level(400, 400);
        let mut refr = seeded_rand(9);
        let c1x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c1y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        let c2x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c2y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        // Rock pixel at (c1x, c1y) — inside trial-1's box, outside trial-2's box.
        assert!(
            !(c2x - 3 <= c1x && c1x < c2x + 3 && c2y - 4 <= c1y && c1y < c2y + 4),
            "fixture: the rock must fall outside trial-2's box"
        );
        level.material_flags[9] = MAT_ROCK;
        level.material_id[(c1x + c1y * level.width) as usize] = 9;

        let mut worms = vec![dead_worm_at(0, 1000, 1500), dead_worm_at(1, 3000, 3000)];
        let mut rand = seeded_rand(9);
        run_begin_respawn(&mut worms, 0, &level, &mut rand);

        assert_eq!(rand.last(), refr.last(), "rock reject => 2nd trial => 4 draws");
        assert_eq!(worms[0].pos, Vec2::new(itof(c2x), itof(c2y)));
    }

    #[test]
    fn t4_begin_respawn_dropdown_is_rand_free_and_moves_pos_down() {
        // Floor level: rows [0, FLOOR) Background, rows [FLOOR, height) solid. Any
        // candidate slides down until pos.y+4 hits the floor. The drop-down draws
        // NO rand => still exactly 2 draws despite moving pos.y.
        let width = 400;
        let height = 250;
        let floor = 200; // rows >= 200 are solid
        let mut level = flat_level(width, height);
        level.material_flags[8] = MAT_BACKGROUND;
        for y in 0..floor {
            for x in 0..width {
                level.material_id[(x + y * width) as usize] = 8;
            }
        }
        let mut refr = seeded_rand(5);
        let c1x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let _c1y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;

        let mut worms = vec![dead_worm_at(0, 1000, 1900), dead_worm_at(1, 3000, 3000)];
        let mut rand = seeded_rand(5);
        run_begin_respawn(&mut worms, 0, &level, &mut rand);

        assert_eq!(rand.last(), refr.last(), "drop-down draws NO rand => still 2 draws");
        // Slid down: y+4 first hits solid at y == floor - 4.
        assert_eq!(ftoi(worms[0].pos.y), floor - 4, "slid down to rest on the floor");
        assert_eq!(ftoi(worms[0].pos.x), c1x, "x is unchanged by the drop-down");
        assert_eq!(worms[0].killed_timer, -1);
    }

    #[test]
    fn t4_begin_respawn_single_worm_enemy_defaults_to_death_pos() {
        // Only ONE worm => `worms.len() != 2` => enemy = temp (the death pos), NOT
        // a second worm. Death pos ON trial-1's candidate makes the enemy(=temp)
        // clause reject it => a 2nd trial. Proves the `worms.size() == 2` gate and
        // the enemy=temp fallback.
        let level = flat_level(400, 400);
        let mut refr = seeded_rand(21);
        let c1x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c1y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        let c2x = TSPAWN_X + refr.bound(TSPAWN_W as u32) as i32;
        let c2y = TSPAWN_Y + refr.bound(TSPAWN_H as u32) as i32;
        // `min_enemy = 0`: enemy(=death pos) rejects only the exact candidate.
        assert!((c1x, c1y) != (c2x, c2y), "fixture: trial-2 differs from trial-1");

        let mut worms = vec![dead_worm_at(0, c1x, c1y)];
        let mut rand = seeded_rand(21);
        begin_respawn(
            &mut worms, 0, &level, TSPAWN_X, TSPAWN_Y, TSPAWN_W, TSPAWN_H, TMIN_LAST, 0, &mut rand,
        );

        assert_eq!(rand.last(), refr.last(), "len!=2: enemy=temp; trial-1 rejected => 4 draws");
        assert_eq!(worms[0].pos, Vec2::new(itof(c2x), itof(c2y)));
        assert_eq!(
            worms[0].logic_respawn,
            Vec2::new(c1x - 80, c1y - 80),
            "logic_respawn still = death - (80,80)"
        );
    }

    #[test]
    fn t4_begin_respawn_trials_guard_breaks_at_50000() {
        // A level that ALWAYS rejects (enemy blankets the whole spawn rect) makes
        // the loop run until the `++trials >= 50000` guard breaks (:736-738) and
        // still sets killed_timer = -1. Enemy huge min-dist so every candidate in
        // [X, X+W) x [Y, Y+H) is within the enemy radius.
        let level = flat_level(600, 600);
        // Enemy in the middle of the spawn rect with a min-dist covering all of it.
        let mut worms = vec![dead_worm_at(0, 1000, 1500), dead_worm_at(1, 150, 140)];
        let big = 100_000; // radius >> the whole 100x80 spawn rect
        let mut rand = seeded_rand(3);
        begin_respawn(
            &mut worms, 0, &level, TSPAWN_X, TSPAWN_Y, TSPAWN_W, TSPAWN_H, TMIN_LAST, big, &mut rand,
        );
        // 50000 trials x 2 draws each = 100000 draws consumed; loop broke, not hung.
        assert_eq!(worms[0].killed_timer, -1, "killed_timer = -1 even on the guard break");
    }

    #[test]
    fn t4_check_respawn_position_reject_clauses_and_raw_old_x_bug() {
        // Big level so the box scan never clamps or reads OOB.
        let level = flat_level(1400, 700);
        // (a) last-pos reject uses the RAW old_x (C++ bug, game.cpp:614): old_x
        // small (<= last) + old_y near y => reject, EVEN THOUGH candidate x is 990
        // px from old_x. Enemy far.
        assert!(
            !check_respawn_position(&level, 9999, 9999, 10, 500, 1000, 505, 30, 30),
            "kDeltaX = old_x (10) <= 30 && |old_y - y| = 5 <= 30 => reject despite x far apart"
        );
        // (b) the bug's tell: a LARGE old_x makes the last clause false even when
        // the candidate sits 5 px from the old position => accepted.
        assert!(
            check_respawn_position(&level, 9999, 9999, 1000, 500, 1005, 505, 30, 30),
            "old_x = 1000 > 30 => last clause false; enemy far; no rock => accept"
        );
        // (c) enemy reject: candidate within min_enemy of the enemy.
        assert!(
            !check_respawn_position(&level, 200, 200, 9999, 9999, 210, 205, 30, 30),
            "|x2-x|=10, |y2-y|=5 <= 30 => enemy reject"
        );
        // (d) rock reject: a rock pixel inside the [x-3,x+3)x[y-4,y+4) box.
        let mut rlevel = flat_level(1400, 700);
        rlevel.material_flags[9] = MAT_ROCK;
        rlevel.material_id[(200 + 200 * 1400) as usize] = 9; // (200,200), in the box
        assert!(
            !check_respawn_position(&rlevel, 9999, 9999, 9999, 9999, 200, 200, 30, 30),
            "rock in the box => reject"
        );
        // (e) accept: clear of last (old_x large), enemy, and rock.
        assert!(
            check_respawn_position(&level, 9999, 9999, 9999, 9999, 200, 200, 30, 30),
            "clear of last, enemy, and rock => accept"
        );
    }

    // ------------------------------------------------------------------------
    // Slice 5d T5: DoRespawning (worm.cpp:755-809) — the drop-in convergence
    // walk (+/-1 four times/tick, no rand), the LimitXy clamp, and — on
    // convergence (+/-5) AND `ready` — the dirt puff then the lone no-arg
    // `rand() & 1` aiming reset + the KillEmAll health restore.
    // ------------------------------------------------------------------------

    // A 1-frame 16x16 all-zero sprite bank: the dirt puff's mask cells are all
    // "other" (value 0) so it writes NOTHING, but it still consumes its one
    // rand(rframe) draw — which is all these tests observe about it.
    fn dirt_sprites() -> SpriteSet {
        SpriteSet {
            width: 16,
            height: 16,
            count: 1,
            data: vec![0u8; 256],
        }
    }

    // rframe=1 => the dirt puff draws exactly one rand(1) (result 0, but a real
    // MT draw) before the aiming rand. sframe=mframe=0 index the lone frame.
    fn dirt_texture() -> Texture {
        Texture {
            sframe: 0,
            rframe: 1,
            mframe: 0,
            ndrawback: false,
        }
    }

    // A dead worm parked at integer death pos `(px, py)` with `logic_respawn` at
    // `logic` and the given `ready`. killed_timer < 0 (the DoRespawning arm).
    fn respawn_worm(px: i32, py: i32, logic: (i32, i32), ready: bool) -> WormState {
        let mut w = WormState::from_init(&two_worms()[0]);
        w.visible = false;
        w.killed_timer = -1;
        w.pos = Vec2::new(itof(px), itof(py));
        w.logic_respawn = Vec2::new(logic.0, logic.1);
        w.ready = ready;
        w
    }

    #[test]
    fn t5_do_respawning_steps_logic_respawn_four_times_by_one_no_rand() {
        // pos=(150,150) => target = Ftoi(pos)-80 = (70,70). Start x below, y above
        // the target so BOTH +/-1 directions are exercised. Not within +/-5 yet =>
        // no draw, no respawn, no rand.
        let sprites = dirt_sprites();
        let tex = [dirt_texture()];
        let mut level = flat_level(400, 400);
        let mut w = respawn_worm(150, 150, (0, 200), true);
        let mut rand = seeded_rand(999);

        do_respawning(&mut w, &mut level, &sprites, &tex, 100, &mut rand);

        // 4 steps: x 0->4 (++), y 200->196 (--). No rand consumed.
        assert_eq!(
            w.logic_respawn,
            Vec2::new(4, 196),
            "logic_respawn stepped +/-1 four times toward Ftoi(pos)-80, per axis"
        );
        assert_eq!(rand.last(), 0, "not converged => zero rand draws");
        assert!(!w.visible, "not converged => no respawn (still invisible)");
        assert!(w.ready, "not converged => ready untouched");
    }

    #[test]
    fn t5_do_respawning_limit_xy_clamps_logic_respawn_both_bounds() {
        // level 200x200 => LimitXy bounds are [0, 42] on both axes. ready=false so
        // the clamp is observed in isolation (no draw even when converged).
        let sprites = dirt_sprites();
        let tex = [dirt_texture()];

        // Upper clamp: logic far above => 4 `--` steps still >> 42 => clamp to 42.
        let mut level = flat_level(200, 200);
        let mut w = respawn_worm(150, 150, (1000, 1000), false);
        let mut rand = seeded_rand(1);
        do_respawning(&mut w, &mut level, &sprites, &tex, 100, &mut rand);
        assert_eq!(w.logic_respawn, Vec2::new(42, 42), "clamped to [.,width-158]=42");
        assert_eq!(rand.last(), 0, "ready=false => no draw even when (clamped) converged");
        assert!(!w.visible, "ready=false => no respawn");

        // Lower clamp: logic far below 0 => 4 `++` steps still < 0 => clamp to 0.
        let mut level2 = flat_level(200, 200);
        let mut w2 = respawn_worm(150, 150, (-1000, -1000), false);
        let mut rand2 = seeded_rand(1);
        do_respawning(&mut w2, &mut level2, &sprites, &tex, 100, &mut rand2);
        assert_eq!(w2.logic_respawn, Vec2::new(0, 0), "clamped to [0,.]");
    }

    #[test]
    fn t5_do_respawning_converged_but_not_ready_does_nothing() {
        // Already at target (70,70) so the steps are no-ops => converged, but
        // ready=false => no dirt draw, no respawn (the "Don't spawn in quicksim"
        // / Fire-not-yet-pressed gate).
        let sprites = dirt_sprites();
        let tex = [dirt_texture()];
        let mut level = flat_level(400, 400);
        let mut w = respawn_worm(150, 150, (70, 70), false);
        let mut rand = seeded_rand(7);

        do_respawning(&mut w, &mut level, &sprites, &tex, 100, &mut rand);

        assert_eq!(rand.last(), 0, "converged but !ready => zero rand draws");
        assert!(!w.visible, "converged but !ready => no respawn");
        assert_eq!(w.logic_respawn, Vec2::new(70, 70), "already at target, unmoved");
    }

    #[test]
    fn t5_do_respawning_converged_and_ready_draws_dirt_then_aiming_bit_both_branches() {
        // Converged (logic == target (70,70)) AND ready => the completion fires:
        // dirt puff (one rand(rframe)) THEN the lone no-arg `rand() & 1` picking
        // the facing. Sweep seeds to force BOTH branches; assert every mutation.
        let mut seen_odd = false; // bit==1 -> Itof(32), direction 0
        let mut seen_even = false; // bit==0 -> Itof(96), direction 1
        for seed in 0u32..64 {
            // Reference stream: dirt draw = rand(1) FIRST, then the raw no-arg
            // draw whose LOW bit picks the facing (C++ `game.rand() & 1`).
            let mut refr = seeded_rand(seed);
            refr.bound(1);
            let bit = refr.next_u32() & 1;
            let expected_last = refr.last();

            let sprites = dirt_sprites();
            let tex = [dirt_texture()];
            let mut level = flat_level(400, 400);
            let mut w = respawn_worm(150, 150, (70, 70), true);
            // Non-default pre-state so the resets are observable.
            w.fire_cone = 99;
            w.vel = Vec2::new(5, 5);
            w.health = 1;
            w.aiming_angle = 12345;
            w.direction = 9;
            let mut rand = seeded_rand(seed);

            do_respawning(&mut w, &mut level, &sprites, &tex, 100, &mut rand);

            // Exactly 2 draws, in order: dirt puff THEN the lone aiming rand.
            assert_eq!(
                rand.last(),
                expected_last,
                "dirt rand(rframe) THEN the lone no-arg rand()&1 (exactly 2 draws, in order)"
            );
            assert!(!w.ready, "ready cleared (:788)");
            assert!(w.visible, "visible = true (:791)");
            assert_eq!(w.fire_cone, 0, "fire_cone = 0 (:792)");
            assert_eq!(w.vel, Vec2::zero(), "vel.Zero() (:793)");
            assert_eq!(w.health, 100, "health = settings_health, KillEmAll restore (:794-796)");
            if bit != 0 {
                assert_eq!(w.aiming_angle, itof(32), "odd bit => Itof(32)");
                assert_eq!(w.direction, 0, "odd bit => direction 0");
                seen_odd = true;
            } else {
                assert_eq!(w.aiming_angle, itof(96), "even bit => Itof(96)");
                assert_eq!(w.direction, 1, "even bit => direction 1");
                seen_even = true;
            }
        }
        assert!(
            seen_odd && seen_even,
            "both `rand() & 1` branches (odd->Itof(32)/dir0, even->Itof(96)/dir1) exercised"
        );
    }

    #[test]
    fn t5_do_respawning_aiming_uses_raw_low_bit_not_bound2_high_bit() {
        // The C++ call form is `game.rand() & 1` (raw next draw, LOW bit), NOT
        // `rand.bound(2)` (Lemire's HIGH bit). They advance the RNG identically
        // (one draw) but generally select DIFFERENT bits, so the call form is
        // load-bearing. Find a seed where the two disagree and pin that the
        // implementation follows the LOW bit.
        let seed = (0u32..10_000)
            .find(|&s| {
                let mut a = seeded_rand(s);
                a.bound(1); // consume the dirt draw
                let raw = a.next_u32();
                let low = raw & 1;
                // bound(2) on the SAME draw would be the high bit:
                let high = ((raw as u64 * 2) >> 32) as u32;
                low != high
            })
            .expect("a seed where low-bit and bound(2) disagree must exist");

        let mut refr = seeded_rand(seed);
        refr.bound(1);
        let low = refr.next_u32() & 1;

        let sprites = dirt_sprites();
        let tex = [dirt_texture()];
        let mut level = flat_level(400, 400);
        let mut w = respawn_worm(150, 150, (70, 70), true);
        let mut rand = seeded_rand(seed);
        do_respawning(&mut w, &mut level, &sprites, &tex, 100, &mut rand);

        // The facing must follow the LOW bit (odd => Itof(32)/dir0).
        if low != 0 {
            assert_eq!(w.aiming_angle, itof(32));
            assert_eq!(w.direction, 0);
        } else {
            assert_eq!(w.aiming_angle, itof(96));
            assert_eq!(w.direction, 1);
        }
    }
}
