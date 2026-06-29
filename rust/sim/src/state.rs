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
use sim_core::fixed::Fixed;
use sim_core::rng::Rand;
use sim_core::tables::precompute_cossin;
use sim_core::vec::Vec2;

use crate::control::{
    process_aiming, process_movement, process_tasks, process_weapon_change, process_weapons,
    ControlConsts,
};
use crate::nobject::{nobject_process, NObjectOutcome};
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bonus {
    pub x: i32,
    pub y: i32,
    pub timer: i32,
    pub weapon: i32,
    pub frame: i32,
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
            cycles,
            ..
        } = self;
        let h_signed_recoil = *h_signed_recoil;
        let settings_loading_time = *settings_loading_time;
        let load_change = *load_change;
        let blood = *blood;
        // The object loops read `cycles` as a value for the `cycles % delay` /
        // `cycles & 7` gates inside `nobject_process`. They must see the value left by
        // the PREVIOUS tick's increment (cycles=k-1 on tick k) — exactly as the C++
        // object loops run BEFORE `++cycles` (game.cpp:357). So snapshot the value
        // here, run the loops with it, then `++cycles` after the loops (see below).
        let cycles_now = *cycles;

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
            match wobject_process(&mut obj, level, weapon, rand) {
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
                cycles_now,
                blood,
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
        // bobjects: no-op this slice (empty pool; BObject::Process not ported).

        // `++cycles` at the exact `game.cpp:357` point — AFTER the four object loops,
        // BEFORE the worm loop. The object loops above ran with `cycles_now` (the
        // value left by tick k-1's increment); after this the worm loop and the
        // tick-end master hash see cycles=k. `cycles` folds into the master
        // `HashGameState` only (hash.rs:50), NOT into any component hash, so advancing
        // it perturbs only the master column of the goldens. Must match the C++ dumper
        // exactly — the off-by-one is load-bearing for the `cycles % delay` gates read
        // DURING the object loop.
        *cycles = cycles.wrapping_add(1);

        for (i, w) in worms.iter_mut().enumerate() {
            // Interleave: apply this worm's input (≈ `Unpack`), then Process it.
            if let Some(input) = inputs.get(i) {
                w.control_states = *input;
            }

            // PARTIAL port of `Worm::Process` (worm.cpp:210-451). The full C++
            // structure is:
            //   health = min(health, settings_health);          // 213 — ALWAYS
            //   if ((mode != KillEmAll && mode != Scales) || lives > 0) {  // 215
            //     if (visible) { ...active-sim body (steps 2-11)... }      // 218
            //     else { steerable_count = 0; PressedOnce(kFire)->ready;   // 431-450
            //            --killed_timer; BeginRespawn; DoRespawning; }
            //   }
            // We port ONLY the `if (visible)` active-simulation arm below. The
            // health=min clamp (213), the game-mode/lives gate (215), and the
            // entire dead-worm `else` arm (431-450) are DELIBERATELY UNPORTED:
            // they are inert while no worm dies/respawns (health == settings_health,
            // no kFire on an idle invisible worm, hash stays CONSTANT), so slices
            // 1-4a match bit-exact. FORWARD-NOTE / latent bug: the first slice that
            // drives a DEAD or respawning worm MUST port the `else` arm (killed_timer
            // countdown + BeginRespawn/DoRespawning) AND the health clamp + lives
            // gate — without it a Rust dead worm never counts down and never respawns
            // (hash diverges). See slice-3 reko (settings_health + visible/dead split).
            if w.visible {
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
            }
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
}
