//! Port of `CheckBonusSpawnPosition` (`game.cpp:200-214`) + `Game::CreateBonus`
//! (`game.cpp:216-265`) — the desync-sensitive bonus spawn (Slice 5c Task 2).
//!
//! `create_bonus` is reached from the per-tick bonus-drop roll in
//! [`crate::state::SimState::process_frame`] (`game.cpp:359-362`), once the gate
//! `!h[HBonusDisable] && max_bonuses > 0 && rand(CBonusDropChance) == 0` opens.
//!
//! ## The RNG order (the contract)
//!
//! It is a **variable-trial position search** followed by a **variable-draw
//! weapon reject loop** — both desync-sensitive (the trial / reject counts depend
//! on the live level + weapon table). Per trial the draws are, in source order:
//!
//! 1. `rand(BonusSpawnRectW)` — the candidate x (`game.cpp:224`).
//! 2. `rand(BonusSpawnRectH)` — the candidate y (`game.cpp:225`).
//!
//! repeated until a candidate passes [`check_bonus_spawn_position`] (a 5×5
//! DirtRock-clear box). On the winning trial:
//!
//! 3. `rand(2)` — the bonus `frame` (`game.cpp:240`), **only** when neither the
//!    `HBonusOnlyHealth` nor `HBonusOnlyWeapon` hack forces it.
//! 4. `rand(bonus_rand_timer[frame][1])` — the spawn timer base (`game.cpp:252`).
//! 5. **if `frame == 0`** (a weapon bonus): the reject loop
//!    `rand(weapons.len())` repeated while `weap_table[w] == 2` (`game.cpp:256-258`).
//! 6. `sobject_types[7].Create(ix, iy, 0)` — the spawn-flash, with its own
//!    sound/dirt RNG cluster via the already-ported [`sobject_create`].
//!
//! `CheckBonusSpawnPosition` draws NO rand. The `HBonus*` hacks are ported guarded
//! but are **false** in the openliero TC, so this TC always draws `rand(2)` for the
//! frame and never offsets the placement rect.

use assets::object::{NObjectType, SObjectType, Weapon};
use assets::sprite::SpriteSet;
use assets::tc::Texture;
use sim_core::fixed::{ftoi, itof};
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::pool::Pool;
use crate::sobject::sobject_create;
use crate::state::{Bonus, LevelSim, NObject, SObject, WObject, WormState};

/// Port of `CheckBonusSpawnPosition` (`game.cpp:200-214`). Draws **NO rand**.
///
/// Builds `Rect(x-2, y-2, x+3, y+3)`, intersects it with the level bounds
/// (`Rect::Intersect` = `max(x1)/max(y1)/min(x2)/min(y2)`), then scans the
/// clamped box. Returns `false` if **any** cell is `DirtRock` (the 5×5 box must be
/// clear), else `true`. The C++ loop is `cx` outer, `cy` inner; there is no RNG so
/// the order is immaterial, but it is mirrored anyway. Every scanned `(cx, cy)` is
/// inside the level (the rect was clamped to `[0, width) × [0, height)`), so
/// [`LevelSim::dirt_rock`]'s in-bounds gate is always satisfied — equivalent to the
/// C++ unchecked `level.Mat(cx, cy).DirtRock()`.
pub fn check_bonus_spawn_position(level: &LevelSim, x: i32, y: i32) -> bool {
    // Rect(x-2, y-2, x+3, y+3) ∩ Bounds(0, 0, width, height).
    let x1 = (x - 2).max(0);
    let y1 = (y - 2).max(0);
    let x2 = (x + 3).min(level.width);
    let y2 = (y + 3).min(level.height);

    for cx in x1..x2 {
        for cy in y1..y2 {
            if level.dirt_rock(cx, cy) {
                return false;
            }
        }
    }
    true
}

/// Port of `Game::CreateBonus` (`game.cpp:216-265`). See the module docs for the
/// exact `rand()` order — it is the contract.
///
/// The state is threaded in rather than held on a `Game`: `bonuses` is the target
/// pool (`NewObject` = [`Pool::spawn`], lowest-free-index), `level` feeds both
/// [`check_bonus_spawn_position`] and the flash's level writes, and the remaining
/// args are the [`sobject_create`] bundle plus the bonus constants/hacks. The
/// `HBonus*` hacks are ported guarded (a TC that sets them works) but inert in this
/// TC. `weapons` doubles as the `rand(weapons.len())` bound and the flash's weapon
/// table.
#[allow(clippy::too_many_arguments)]
pub fn create_bonus(
    bonuses: &mut Pool<Bonus>,
    level: &mut LevelSim,
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    weapons: &[Weapon],
    nobject_types: &[NObjectType],
    sobject_types: &[SObjectType],
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    textures: &[Texture],
    blood: i32,
    max_bonuses: i32,
    bonus_spawn_rect_w: i32,
    bonus_spawn_rect_h: i32,
    bonus_spawn_rect_x: i32,
    bonus_spawn_rect_y: i32,
    h_bonus_spawn_rect: bool,
    h_bonus_only_health: bool,
    h_bonus_only_weapon: bool,
    bonus_rand_timer: &[[i32; 2]; 2],
    weap_table: &[i32],
    rand: &mut Rand,
) {
    // :219 `if (bonuses.Size() >= settings->max_bonuses) return;` — no rand.
    if bonuses.len() as i32 >= max_bonuses {
        return;
    }

    // :223 the variable-trial position search (capped at 50000 trials).
    for _ in 0..50000 {
        // :224-225 per-trial placement draws — ALWAYS two, in this order.
        let mut ix = rand.bound(bonus_spawn_rect_w as u32) as i32;
        let mut iy = rand.bound(bonus_spawn_rect_h as u32) as i32;

        // :227-230 HBonusSpawnRect offset (no rand). Inert in this TC.
        if h_bonus_spawn_rect {
            ix += bonus_spawn_rect_x;
            iy += bonus_spawn_rect_y;
        }

        // :232 the clear-ground test (no rand).
        if check_bonus_spawn_position(level, ix, iy) {
            // :233-241 frame: forced by a hack, else `rand(2)` (draw 3). The hack
            // branches are guarded but false in this TC ⇒ the `rand(2)` always draws.
            let frame = if h_bonus_only_health {
                1
            } else if h_bonus_only_weapon {
                0
            } else {
                rand.bound(2) as i32
            };

            // :243-246 NewObject; if the pool is full, bail (matching `if (!bonus)
            // return;`). The frame draw above has ALREADY happened — its rand is
            // consumed even on this bail, exactly as in C++.
            let slot = match bonuses.spawn(Bonus {
                x: itof(ix),
                y: itof(iy),
                vel_y: 0,
                frame,
                timer: 0,  // set after the timer draw below
                weapon: 0, // :253; possibly overwritten by the reject loop
            }) {
                Some(s) => s,
                None => return,
            };

            // :252 timer = rand(range) + base (draw 4), AFTER NewObject.
            let timer = rand.bound(bonus_rand_timer[frame as usize][1] as u32) as i32
                + bonus_rand_timer[frame as usize][0];

            // :255-259 weapon-bonus reject loop: draw `rand(weapons.len())` while the
            // drawn weapon is banned (`weap_table[w] == 2`) — a variable number of
            // draws. Only for `frame == 0` (a weapon bonus); a health bonus keeps 0.
            let mut weapon = 0;
            if frame == 0 {
                loop {
                    weapon = rand.bound(weapons.len() as u32) as i32;
                    if weap_table[weapon as usize] != 2 {
                        break;
                    }
                }
            }

            let b = bonuses.get_mut(slot).expect("just-spawned bonus slot is live");
            b.timer = timer;
            b.weapon = weapon;

            // :261 the spawn-flash: sobject_types[7].Create(ix, iy, 0). owner_idx 0
            // matches the C++ `0` arg. Draws its own sound/dirt RNG cluster.
            sobject_create(
                &sobject_types[7],
                ix,
                iy,
                0,
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
                rand,
            );
            return;
        }
    }
}

/// The driver's verdict for a processed bonus: whether the slot should be freed.
/// Returned by [`bonus_process`] so the slot-walk in [`process_bonuses`] (and the
/// `process_frame` bonuses loop) can free-during-iteration the C++ way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BonusOutcome {
    /// Keep the bonus in its slot (write the mutated copy back).
    Keep,
    /// Free the slot (the `--timer<=0 && used` expire path).
    Free,
}

/// Port of `Bonus::Process` (`bonus.cpp:6-35`). The fall/bounce path draws **NO
/// rand**; only the expiry sobject (`sobject_create`, reached on `--timer<=0`)
/// draws. Returns [`BonusOutcome::Free`] when the bonus expired AND `used` (so the
/// driver frees the slot), else [`BonusOutcome::Keep`].
///
/// The arithmetic is the bit-exact contract:
/// * `:9` `y += vel_y` (16.16 fixed, wrapping).
/// * `:16-18` standing over air (`Inside(kIx,kIy+1) && Mat(kIx,kIy+1).Background()`)
///   → `vel_y += BonusGravity`. The `Inside` gate is replicated before the
///   (unchecked) [`LevelSim::background`] probe so a negative/OOB cell never reads
///   a wrapped pixel — matching the C++ short-circuit.
/// * `:20-27` bounce: if the *next* row `kInewY` is above the top, at/below
///   `height-1`, or DirtRock, reflect `vel_y = -(vel_y*BounceMul)/BounceDiv`
///   (multiply, negate, then truncating divide), zeroed when `|vel_y| < 100` (a
///   literal, not a const). The `||` short-circuits the bounds terms before the
///   `dirt_rock` probe, so `dirt_rock` is only reached with `0 <= kInewY <
///   height-1` (in bounds) — its `inside` gate is then always satisfied,
///   equivalent to the C++ unchecked `Mat(kIx, kInewY).DirtRock()`.
/// * `:29-34` EXPIRE: `--timer<=0` → spawn `sobject_types[bonus_s_objects[frame]]`
///   at `(kIx, kIy)` via the already-ported [`sobject_create`] (which draws its own
///   sound/dirt RNG), then free the bonus **iff** `used`.
///
/// `used` mirrors the C++ `ExactObjectListBase::used` flag (`exactObjectList.hpp`):
/// the live/free marker. The pool's slot-walk only visits live slots (C++ `All()`
/// skips `!used`), so the driver passes `used = true`; the gate is kept faithful so
/// the **spawn** at expiry is unconditional while the **free** is `used`-gated
/// (passing `used = false` keeps the slot but still spawns the sobject).
#[allow(clippy::too_many_arguments)]
pub fn bonus_process(
    bonus: &mut Bonus,
    used: bool,
    level: &mut LevelSim,
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    bonuses: &mut Pool<Bonus>,
    weapons: &[Weapon],
    nobject_types: &[NObjectType],
    sobject_types: &[SObjectType],
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    textures: &[Texture],
    blood: i32,
    bonus_gravity: i32,
    bonus_bounce_mul: i32,
    bonus_bounce_div: i32,
    bonus_s_objects: &[i32; 2],
    rand: &mut Rand,
) -> BonusOutcome {
    // :9 y += vel_y.
    bonus.y = bonus.y.wrapping_add(bonus.vel_y);

    // :11-12 kIx, kIy (Ftoi == >>16). :14 asserts kIx in [0,width) — debug only.
    let k_ix = ftoi(bonus.x);
    let k_iy = ftoi(bonus.y);

    // :16-18 standing over air → fall. Inside-gated, then the unchecked Background()
    // probe (the gate guarantees the cell is in bounds).
    if level.inside(k_ix, k_iy + 1) && level.background(k_ix, k_iy + 1) {
        bonus.vel_y = bonus.vel_y.wrapping_add(bonus_gravity);
    }

    // :20-27 bounce off terrain / top / bottom bound.
    let k_inew_y = ftoi(bonus.y.wrapping_add(bonus.vel_y));
    if k_inew_y < 0 || k_inew_y >= level.height - 1 || level.dirt_rock(k_ix, k_inew_y) {
        // -(vel_y * BounceMul) / BounceDiv: multiply, negate, then truncating divide.
        bonus.vel_y = bonus
            .vel_y
            .wrapping_mul(bonus_bounce_mul)
            .wrapping_neg()
            .wrapping_div(bonus_bounce_div);
        // :24-26 `if (abs(vel_y) < 100) vel_y = 0;` — 100 is a literal, not a const.
        if bonus.vel_y.abs() < 100 {
            bonus.vel_y = 0;
        }
    }

    // :29-34 EXPIRE: --timer<=0.
    bonus.timer = bonus.timer.wrapping_sub(1);
    if bonus.timer <= 0 {
        // :30 spawn sobject_types[bonus_s_objects[frame]] at (kIx, kIy). Draws its
        // own sound/dirt RNG cluster inside sobject_create. ALWAYS runs at expiry,
        // regardless of `used`.
        sobject_create(
            &sobject_types[bonus_s_objects[bonus.frame as usize] as usize],
            k_ix,
            k_iy,
            0,
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
            rand,
        );
        // :31-33 free the bonus iff used.
        if used {
            return BonusOutcome::Free;
        }
    }
    BonusOutcome::Keep
}

/// The bonuses Process loop (`game.cpp:287-290`): walks the `bonuses` pool in slot
/// (index) order — matching `ExactObjectList::All()`, which skips free slots — and
/// runs [`bonus_process`] on each live bonus, freeing the slot on
/// [`BonusOutcome::Free`].
///
/// Free-during-iteration is the C++ contract: `Bonus::Process` may `Free(this)` on
/// the expire path. Walking by index and copying each bonus out by value lets us
/// hand `&mut` of the *other* pools to `bonus_process` (its expiry `sobject_create`)
/// and free the current slot without disturbing the walk — exactly as the C++
/// `Range` already advanced its cursor past the returned object before `Process`
/// runs, so freeing the just-returned slot is safe. The same shape as the
/// `process_frame` sobjects/wobjects/nobjects slot-walks.
#[allow(clippy::too_many_arguments)]
pub fn process_bonuses(
    bonuses: &mut Pool<Bonus>,
    level: &mut LevelSim,
    worms: &mut [WormState],
    wobjects: &mut Pool<WObject>,
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    weapons: &[Weapon],
    nobject_types: &[NObjectType],
    sobject_types: &[SObjectType],
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    textures: &[Texture],
    blood: i32,
    bonus_gravity: i32,
    bonus_bounce_mul: i32,
    bonus_bounce_div: i32,
    bonus_s_objects: &[i32; 2],
    rand: &mut Rand,
) {
    for slot in 0..bonuses.capacity() {
        let obj_ref = match bonuses.get(slot) {
            Some(o) => o,
            None => continue,
        };
        let mut obj = *obj_ref;
        // A live slot ⇒ `used == true` (C++ `All()` only yields used bonuses).
        match bonus_process(
            &mut obj,
            true,
            level,
            worms,
            wobjects,
            nobjects,
            sobjects,
            bonuses,
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
        ) {
            BonusOutcome::Keep => {
                *bonuses.get_mut(slot).expect("slot still live") = obj;
            }
            BonusOutcome::Free => {
                bonuses.free(slot);
            }
        }
    }
}

/// Port of `Game::DoHealingDirect` (`game.cpp:555-565`), KillEmAll path — RNG-free.
///
/// `w.health += amount`, then clamp to `settings_health` (the `else` arm). The
/// `kGmScalesOfJustice` branch (which converts overflow health into extra lives)
/// is present-but-guarded in the plan — the openliero TC is always KillEmAll, so
/// only the clamp arm is modelled (`game_mode` is unmodelled in the sim). A
/// negative `amount` is possible in general but the pickup only ever passes a
/// non-negative heal.
pub fn do_healing_direct(w: &mut WormState, amount: i32, settings_health: i32) {
    w.health += amount;
    w.health = w.health.min(settings_health);
}

/// Port of the visible-worm **bonus pickup** block (`worm.cpp:287-322`) — runs in
/// `Worm::Process` right after the reaction-force block (`:285`) and before
/// `ProcessSteerables`. Walks `game.bonuses.All()` (slot order, live slots only)
/// and, for each bonus whose 11×11 AABB overlaps the worm, applies the health /
/// weapon / booby branch.
///
/// ## The RNG order (the contract, verified against `worm.cpp:287-322`)
///
/// `ipos = Ftoi(pos)` is computed once (`:285`); `pos` is not moved by the pickup,
/// so the C++ single evaluation is preserved. Per in-range bonus, in source order:
///
/// * **frame == 1 (health)** (`:291-297`): iff `health < settings_health`, `Free`
///   the bonus, then draw **one** `rand(BonusHealthVar)` and
///   `DoHealingDirect((rand + BonusMinHealth) * settings_health / 100)`. When
///   `health >= settings_health` there is **no free and no draw**. `health` is read
///   fresh each iteration, so a heal from an earlier bonus in the same walk feeds
///   the next gate (see the running-health test).
/// * **frame == 0 (weapon)** (`:298-319`): **always** draw `rand(BonusExplodeRisk)`.
///   * `> 1` → reload (`:300-313`): unless `HBonusReloadOnly`, set `fire_cone = 0`
///     and `ww.type = &weapons[bonus.weapon]` / `ww.ammo = ww.type->ammo` (the
///     hashed weapon id + its starting ammo); then `Free` the bonus and
///     `ww.loading_left = 0`. No further draw. (The `SoundReloaded` play is
///     sound-only and omitted.)
///   * `<= 1` → booby (`:314-318`): capture `Ftoi(bonus.x/y)`, `Free`, then
///     `sobject_types[0].Create(kBix, kBiy, index, nullptr)` via [`sobject_create`]
///     (its own sound/dirt RNG cluster). The bonus pool is threaded through
///     `sobject_create` (Slice 6 T3), so the booby's own chain-loop is live: a
///     bonus in the booby's `detect_range` chains further. `Free` runs before the
///     `Create`, so the just-picked bonus is never re-detected by the booby scan.
///
/// The box test / `HBonus*` guards match the C++ exactly; the block is inert
/// (empty pool) for slices 1-5c, keeping their goldens byte-identical.
#[allow(clippy::too_many_arguments)]
pub fn worm_pickup_bonuses(
    worms: &mut [WormState],
    wi: usize,
    bonuses: &mut Pool<Bonus>,
    wobjects: &mut Pool<WObject>,
    nobjects: &mut Pool<NObject>,
    sobjects: &mut Pool<SObject>,
    weapons: &[Weapon],
    nobject_types: &[NObjectType],
    sobject_types: &[SObjectType],
    level: &mut LevelSim,
    cossin: &[Vec2; 128],
    large_sprites: &SpriteSet,
    textures: &[Texture],
    blood: i32,
    settings_health: i32,
    bonus_health_var: i32,
    bonus_min_health: i32,
    bonus_explode_risk: i32,
    h_bonus_reload_only: bool,
    rand: &mut Rand,
) {
    // :285 ipos = Ftoi(pos), computed once — the pickup never moves the worm, so
    // pos is stable across the walk (matching the single C++ evaluation).
    let ipos_x = ftoi(worms[wi].pos.x);
    let ipos_y = ftoi(worms[wi].pos.y);

    // :287-288 walk bonuses in slot (index) order == ExactObjectList::All(). Copy
    // each live bonus out by value so `Free(br)` (a slot free) can run mid-walk
    // without disturbing the cursor — the same shape as `process_bonuses`.
    for slot in 0..bonuses.capacity() {
        let bonus = match bonuses.get(slot) {
            Some(b) => *b,
            None => continue,
        };

        // :289-290 the 11×11 AABB gate — STRICT on all four sides.
        let bx = ftoi(bonus.x);
        let by = ftoi(bonus.y);
        if !(ipos_x + 5 > bx && ipos_x - 5 < bx && ipos_y + 5 > by && ipos_y - 5 < by) {
            continue;
        }

        if bonus.frame == 1 {
            // :291-297 health bonus.
            if worms[wi].health < settings_health {
                // :293 Free BEFORE the heal draw (Free draws no rand).
                bonuses.free(slot);
                // :295 the ONLY draw: rand(BonusHealthVar). The heal amount is
                // integer `(rand + BonusMinHealth) * settings_health / 100`.
                let amount = (rand.bound(bonus_health_var as u32) as i32 + bonus_min_health)
                    * settings_health
                    / 100;
                do_healing_direct(&mut worms[wi], amount, settings_health);
            }
            // health >= settings_health: no free, NO rand.
        } else if bonus.frame == 0 {
            // :298-319 weapon bonus. rand(BonusExplodeRisk) is ALWAYS drawn.
            if rand.bound(bonus_explode_risk as u32) as i32 > 1 {
                // :300-313 reload path.
                let cw = worms[wi].current_weapon as usize;
                if !h_bonus_reload_only {
                    // :303-306 fire_cone = 0, ww.type = &weapons[bonus.weapon],
                    // ww.ammo = ww.type->ammo.
                    worms[wi].fire_cone = 0;
                    let def = &weapons[bonus.weapon as usize];
                    worms[wi].weapons[cw].ty = Some(def.id);
                    worms[wi].weapons[cw].ammo = def.ammo;
                }
                // :309 SoundReloaded play — sound-only, omitted (draws no rand).
                // :311 Free, :313 loading_left = 0.
                bonuses.free(slot);
                worms[wi].weapons[cw].loading_left = 0;
            } else {
                // :314-318 booby path — capture the pixel pos BEFORE Free, then
                // spawn sobject_types[0] at it (its own RNG cluster).
                let kbix = ftoi(bonus.x);
                let kbiy = ftoi(bonus.y);
                bonuses.free(slot);
                let owner_idx = worms[wi].index;
                sobject_create(
                    &sobject_types[0],
                    kbix,
                    kbiy,
                    owner_idx,
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
                    rand,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MAT_BACKGROUND, MAT_ROCK};
    use sim_core::tables::precompute_cossin;

    // A seed whose successive draws are distinct, so an order swap / miscount is
    // detectable.
    const SEED: u32 = 0x5151;

    fn seeded() -> Rand {
        let mut r = Rand::new();
        r.seed(SEED);
        r
    }

    // The spawn-flash sobject (sobject_types[7]). Tuned to draw EXACTLY one rand —
    // the sound `rand(num_sounds)` — and nothing else: `damage = 0` skips the whole
    // worm/blow-away/dirt-throw block, `dirt_effect = -1` skips the carve. So the
    // flash's only RNG footprint is a single `rand(3)`, which the reference streams
    // replay after the bonus draws. It still spawns one SObject (proof Create ran).
    fn flash_sobject() -> SObjectType {
        SObjectType {
            id: 7,
            start_sound: 0,
            num_sounds: 3,
            anim_delay: 2,
            start_frame: 0,
            num_frames: 5,
            detect_range: 0,
            damage: 0,
            blow_away: 0,
            dirt_effect: -1,
            ..Default::default()
        }
    }

    // sobject_types padded so index 7 is the flash; 0..7 are unused placeholders.
    fn sobject_types() -> Vec<SObjectType> {
        let mut v = vec![SObjectType::default(); 8];
        v[7] = flash_sobject();
        v
    }

    // The flash draws one rand(3) iff start_sound >= 0 (it is). Replay it onto a
    // reference stream.
    fn replay_flash(r: &mut Rand) {
        r.bound(3); // sound
    }

    // An all-CLEAR level (material 0 = Background, material 1 = Rock). Every cell is
    // background ⇒ check_bonus_spawn_position passes on the first trial.
    fn clear_level(width: i32, height: i32) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[0] = MAT_BACKGROUND;
        material_flags[1] = MAT_ROCK; // a DirtRock material we can stamp
        LevelSim {
            width,
            height,
            material_id: vec![0u8; (width * height) as usize],
            material_flags,
        }
    }

    fn empty_pools() -> (Pool<WObject>, Pool<NObject>, Pool<SObject>) {
        (Pool::new(600), Pool::new(600), Pool::new(700))
    }

    // A handful of weapons (only the count matters for `rand(weapons.len())`).
    fn weapons(n: usize) -> Vec<Weapon> {
        vec![Weapon::default(); n]
    }

    // The full create_bonus call with the common test plumbing. Returns nothing;
    // the caller inspects the pools + rand.
    #[allow(clippy::too_many_arguments)]
    fn run_create_bonus(
        bonuses: &mut Pool<Bonus>,
        level: &mut LevelSim,
        rand: &mut Rand,
        max_bonuses: i32,
        rect_w: i32,
        rect_h: i32,
        timer: [[i32; 2]; 2],
        weps: &[Weapon],
        weap_table: &[i32],
        hacks: (bool, bool, bool, i32, i32), // spawn_rect, only_health, only_weapon, off_x, off_y
    ) {
        let cossin = precompute_cossin();
        let nts: Vec<NObjectType> = Vec::new();
        let sts = sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let (h_spawn_rect, h_only_health, h_only_weapon, off_x, off_y) = hacks;
        create_bonus(
            bonuses,
            level,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            weps,
            &nts,
            &sts,
            &cossin,
            &SpriteSet::default(),
            &[],
            100,
            max_bonuses,
            rect_w,
            rect_h,
            off_x,
            off_y,
            h_spawn_rect,
            h_only_health,
            h_only_weapon,
            &timer,
            weap_table,
            rand,
        );
        // Stash the flash sobject count via the pool the caller does NOT see; assert
        // it spawned exactly one (proof Create ran) by leaking it into sobjects len.
        assert_eq!(sobjects.len(), 1, "the spawn-flash sobject_types[7] was created");
    }

    // ---- (a) clear-ground trial-1 spawn: exact draw shape -------------------

    #[test]
    fn clear_ground_spawns_on_trial_one_with_exact_draw_order() {
        let rect_w = 40;
        let rect_h = 30;
        let timer = [[100, 50], [200, 70]]; // [base, range] per frame
        let weps = weapons(5);
        let weap_table = vec![0i32; 5]; // none banned -> weapon loop draws exactly once
        let mut level = clear_level(100, 100);
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut rand = seeded();

        // Reference stream: replay the EXACT draw sequence by hand.
        let mut refr = seeded();
        let ex_ix = refr.bound(rect_w as u32) as i32; // draw 1
        let ex_iy = refr.bound(rect_h as u32) as i32; // draw 2
        let ex_frame = refr.bound(2) as i32; // draw 3 (no hacks)
        let ex_timer =
            refr.bound(timer[ex_frame as usize][1] as u32) as i32 + timer[ex_frame as usize][0];
        let mut ex_weapon = 0;
        if ex_frame == 0 {
            // weap_table all 0 -> the do/while runs its body once, never rejects.
            ex_weapon = refr.bound(weps.len() as u32) as i32;
        }
        replay_flash(&mut refr); // the flash's single sound rand
        let expected_last = refr.last();

        run_create_bonus(
            &mut bonuses,
            &mut level,
            &mut rand,
            4,
            rect_w,
            rect_h,
            timer,
            &weps,
            &weap_table,
            (false, false, false, 0, 0),
        );

        assert_eq!(bonuses.len(), 1, "exactly one bonus spawned");
        let b = *bonuses.get(0).expect("bonus in slot 0");
        assert_eq!(b.x, itof(ex_ix), "bonus.x = Itof(ix)");
        assert_eq!(b.y, itof(ex_iy), "bonus.y = Itof(iy)");
        assert_eq!(b.vel_y, 0, "bonus.vel_y = 0");
        assert_eq!(b.frame, ex_frame, "bonus.frame = rand(2)");
        assert_eq!(b.timer, ex_timer, "bonus.timer = rand(range) + base");
        assert_eq!(b.weapon, ex_weapon, "bonus.weapon = rand(weapons.len()) [frame 0]");
        assert_eq!(
            rand.last(),
            expected_last,
            "draw order/count: [rand(W), rand(H), rand(2), rand(timerRange)] (+ weapon loop) + flash"
        );
    }

    // ---- (b) first trial on DirtRock retries --------------------------------

    #[test]
    fn first_trial_on_dirtrock_retries_with_extra_wh_pair() {
        let rect_w = 40;
        let rect_h = 30;
        let timer = [[100, 50], [200, 70]];
        let weps = weapons(5);
        let weap_table = vec![0i32; 5];

        // Predict the first TWO candidate positions from the seed, then stamp the
        // 5×5 box around the FIRST candidate with DirtRock so trial 1 fails and
        // trial 2 (clear) wins. The retry costs an extra rand(W)/rand(H) pair.
        let mut pred = seeded();
        let ix1 = pred.bound(rect_w as u32) as i32;
        let iy1 = pred.bound(rect_h as u32) as i32;
        let ix2 = pred.bound(rect_w as u32) as i32;
        let iy2 = pred.bound(rect_h as u32) as i32;
        // Guard: the two candidates must differ enough that stamping the first box
        // does not also block the second (boxes are 5×5 = radius 2). If they overlap,
        // adjust the seed. For this seed they are distinct.
        assert!(
            (ix1 - ix2).abs() > 4 || (iy1 - iy2).abs() > 4,
            "candidate boxes must not overlap (seed-dependent; pick another seed if they do)"
        );

        let mut level = clear_level(100, 100);
        // Stamp DirtRock (material 1) across the whole 5×5 box of candidate 1.
        for cy in (iy1 - 2)..(iy1 + 3) {
            for cx in (ix1 - 2)..(ix1 + 3) {
                if cx >= 0 && cx < level.width && cy >= 0 && cy < level.height {
                    level.material_id[(cy * level.width + cx) as usize] = 1;
                }
            }
        }

        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut rand = seeded();

        // Reference: TWO W/H pairs (trial 1 rejected), then frame/timer/weapon at
        // candidate 2, then the flash.
        let mut refr = seeded();
        refr.bound(rect_w as u32); // trial 1 ix (rejected position)
        refr.bound(rect_h as u32); // trial 1 iy
        refr.bound(rect_w as u32); // trial 2 ix == ix2
        refr.bound(rect_h as u32); // trial 2 iy == iy2
        let ex_frame = refr.bound(2) as i32;
        let _ = refr.bound(timer[ex_frame as usize][1] as u32);
        if ex_frame == 0 {
            refr.bound(weps.len() as u32);
        }
        replay_flash(&mut refr);
        let expected_last = refr.last();

        run_create_bonus(
            &mut bonuses,
            &mut level,
            &mut rand,
            4,
            rect_w,
            rect_h,
            timer,
            &weps,
            &weap_table,
            (false, false, false, 0, 0),
        );

        assert_eq!(bonuses.len(), 1, "one bonus spawned (on the second, clear trial)");
        let b = *bonuses.get(0).expect("bonus in slot 0");
        assert_eq!(b.x, itof(ix2), "bonus landed at the SECOND candidate x");
        assert_eq!(b.y, itof(iy2), "bonus landed at the SECOND candidate y");
        assert_eq!(
            rand.last(),
            expected_last,
            "the DirtRock first trial drove an extra rand(W)/rand(H) pair"
        );
    }

    // ---- (c) size >= max_bonuses early-out draws nothing --------------------

    #[test]
    fn at_capacity_early_out_draws_no_rand() {
        // max_bonuses == 0: size (0) >= 0 -> return immediately, NO rand drawn, no
        // bonus, no flash. rand.last stays at the post-seed 0.
        let mut level = clear_level(100, 100);
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut rand = seeded();
        assert_eq!(rand.last(), 0, "no draw yet");

        let cossin = precompute_cossin();
        let sts = sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let weps = weapons(5);
        create_bonus(
            &mut bonuses,
            &mut level,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            &weps,
            &[],
            &sts,
            &cossin,
            &SpriteSet::default(),
            &[],
            100,
            0, // max_bonuses == 0 -> early out
            40,
            30,
            0,
            0,
            false,
            false,
            false,
            &[[100, 50], [200, 70]],
            &vec![0i32; 5],
            &mut rand,
        );

        assert_eq!(rand.last(), 0, "early-out draws NO rand");
        assert_eq!(bonuses.len(), 0, "no bonus spawned");
        assert_eq!(sobjects.len(), 0, "no spawn-flash created");
    }

    #[test]
    fn prefilled_pool_at_capacity_early_out() {
        // A non-zero cap that is already met: size (2) >= max_bonuses (2) -> return.
        let mut level = clear_level(100, 100);
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(Bonus::default());
        bonuses.spawn(Bonus::default());
        let mut rand = seeded();

        let cossin = precompute_cossin();
        let sts = sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let weps = weapons(5);
        create_bonus(
            &mut bonuses,
            &mut level,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            &weps,
            &[],
            &sts,
            &cossin,
            &SpriteSet::default(),
            &[],
            100,
            2, // size 2 >= max_bonuses 2
            40,
            30,
            0,
            0,
            false,
            false,
            false,
            &[[100, 50], [200, 70]],
            &vec![0i32; 5],
            &mut rand,
        );

        assert_eq!(rand.last(), 0, "at-capacity early-out draws NO rand");
        assert_eq!(bonuses.len(), 2, "pool unchanged");
        assert_eq!(sobjects.len(), 0, "no spawn-flash created");
    }

    // ---- (d) weapon reject loop skips weap_table == 2 -----------------------

    #[test]
    fn weapon_reject_loop_skips_banned_weapons() {
        // Force frame == 0 via the OnlyWeapon hack (so NO rand(2) frame draw — the
        // hack branch is exercised) and use a weap_table that bans some indices. The
        // do/while re-draws rand(weapons.len()) while the drawn index is banned; the
        // spawned weapon MUST be a non-banned index, and the draw count matches a
        // hand-replayed reject loop.
        let rect_w = 40;
        let rect_h = 30;
        let timer = [[100, 50], [200, 70]];
        let weps = weapons(5);
        // Ban indices 0..=3, allow only index 4: the loop keeps drawing until it hits 4.
        let weap_table = vec![2, 2, 2, 2, 0];
        let mut level = clear_level(100, 100);
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut rand = seeded();

        // Reference: OnlyWeapon forces frame 0 (no rand(2)). Placement, then timer,
        // then the reject loop (draw until weap_table[w] != 2), then the flash.
        let mut refr = seeded();
        refr.bound(rect_w as u32); // ix
        refr.bound(rect_h as u32); // iy
        // frame forced to 0 by the hack -> NO rand(2).
        refr.bound(timer[0][1] as u32); // timer (frame 0)
        let mut ex_weapon;
        loop {
            ex_weapon = refr.bound(weps.len() as u32) as i32;
            if weap_table[ex_weapon as usize] != 2 {
                break;
            }
        }
        replay_flash(&mut refr);
        let expected_last = refr.last();

        run_create_bonus(
            &mut bonuses,
            &mut level,
            &mut rand,
            4,
            rect_w,
            rect_h,
            timer,
            &weps,
            &weap_table,
            (false, false, true, 0, 0), // OnlyWeapon hack on
        );

        assert_eq!(bonuses.len(), 1, "one weapon bonus spawned");
        let b = *bonuses.get(0).expect("bonus in slot 0");
        assert_eq!(b.frame, 0, "OnlyWeapon hack forced frame 0 (no rand(2) drawn)");
        assert_eq!(b.weapon, ex_weapon, "weapon = first non-banned reject-loop draw");
        assert_ne!(
            weap_table[b.weapon as usize], 2,
            "the spawned weapon is NOT banned (weap_table != 2)"
        );
        assert_eq!(
            rand.last(),
            expected_last,
            "the reject loop re-drew rand(weapons.len()) past every banned index"
        );
    }

    // ---- the OnlyHealth hack forces frame 1 (a health bonus, no weapon loop) -

    #[test]
    fn only_health_hack_forces_frame_one_and_no_weapon_loop() {
        let rect_w = 40;
        let rect_h = 30;
        let timer = [[100, 50], [200, 70]];
        let weps = weapons(5);
        let weap_table = vec![0i32; 5];
        let mut level = clear_level(100, 100);
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut rand = seeded();

        // Reference: OnlyHealth forces frame 1 (no rand(2)). Placement, timer (frame
        // 1), NO weapon loop (frame != 0), then the flash.
        let mut refr = seeded();
        refr.bound(rect_w as u32);
        refr.bound(rect_h as u32);
        let ex_timer = refr.bound(timer[1][1] as u32) as i32 + timer[1][0];
        replay_flash(&mut refr);
        let expected_last = refr.last();

        run_create_bonus(
            &mut bonuses,
            &mut level,
            &mut rand,
            4,
            rect_w,
            rect_h,
            timer,
            &weps,
            &weap_table,
            (false, true, false, 0, 0), // OnlyHealth hack on
        );

        let b = *bonuses.get(0).expect("bonus in slot 0");
        assert_eq!(b.frame, 1, "OnlyHealth forced frame 1");
        assert_eq!(b.weapon, 0, "frame 1 -> weapon stays 0 (no reject loop)");
        assert_eq!(b.timer, ex_timer, "timer uses the frame-1 [base, range]");
        assert_eq!(
            rand.last(),
            expected_last,
            "no rand(2), no weapon loop: [rand(W), rand(H), rand(timerRange)] + flash"
        );
    }

    // ---- check_bonus_spawn_position: 5×5 DirtRock scan, no rand --------------

    #[test]
    fn check_position_rejects_any_dirtrock_in_box_and_draws_no_rand() {
        let mut level = clear_level(20, 20);
        // A single DirtRock pixel inside the 5×5 box around (10,10) rejects it.
        level.material_id[(10 * 20 + 11) as usize] = 1; // (11,10), within [8,12]×[8,12]
        assert!(
            !check_bonus_spawn_position(&level, 10, 10),
            "one DirtRock cell in the box -> rejected"
        );
        // A clear box passes.
        assert!(
            check_bonus_spawn_position(&level, 4, 4),
            "a box with no DirtRock -> accepted"
        );
        // The box is clamped to bounds: a position at the corner whose clamped box is
        // all-background still passes.
        assert!(
            check_bonus_spawn_position(&level, 0, 0),
            "corner box clamped to bounds, all clear -> accepted"
        );
    }

    // ---- Bonus::Process (bonus.cpp:6-35) ------------------------------------

    // Run `bonus_process` with the common test plumbing. Returns the outcome plus
    // the sobjects pool so the caller can assert the expiry spawn. `bonus_s_objects`
    // maps both frames to index 7 (the flash) so an expiry spawns the flash.
    #[allow(clippy::too_many_arguments)]
    fn run_bonus_process(
        bonus: &mut Bonus,
        used: bool,
        level: &mut LevelSim,
        rand: &mut Rand,
        gravity: i32,
        mul: i32,
        div: i32,
    ) -> (BonusOutcome, Pool<SObject>) {
        let cossin = precompute_cossin();
        let nts: Vec<NObjectType> = Vec::new();
        let sts = sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        let mut worms: Vec<WormState> = Vec::new();
        let weps = weapons(5);
        let outcome = bonus_process(
            bonus,
            used,
            level,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            &mut bonuses,
            &weps,
            &nts,
            &sts,
            &cossin,
            &SpriteSet::default(),
            &[],
            100,
            gravity,
            mul,
            div,
            &[7, 7],
            rand,
        );
        (outcome, sobjects)
    }

    // ---- (a) over air: falls (vel_y += gravity, y += vel_y), no bounce, no rand --

    #[test]
    fn over_air_falls_with_gravity_and_draws_no_rand() {
        let mut level = clear_level(100, 100); // all background
        let mut rand = seeded();
        // y=Itof(50), vel_y=5000 (sub-pixel). Over air -> gravity adds; far from any
        // floor/bound -> no bounce; timer high -> no expiry, so NO rand is drawn.
        let mut bonus = Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 5000,
            frame: 0,
            timer: 1000,
            weapon: 0,
        };
        let (outcome, sobjects) = run_bonus_process(&mut bonus, true, &mut level, &mut rand, 1000, 2, 3);

        assert_eq!(outcome, BonusOutcome::Keep, "no expiry -> Keep");
        assert_eq!(bonus.y, itof(50) + 5000, "y += vel_y (initial 5000)");
        assert_eq!(bonus.vel_y, 6000, "vel_y += BonusGravity (5000 + 1000)");
        assert_eq!(bonus.timer, 999, "--timer");
        assert_eq!(rand.last(), 0, "the fall path draws NO rand");
        assert_eq!(sobjects.len(), 0, "no expiry sobject (timer not expired)");
    }

    // ---- (b) bounce off floor: vel_y = -(vel_y*Mul)/Div, zeroed if |.|<100 -------

    #[test]
    fn bounce_off_bottom_bound_reflects_velocity() {
        let mut level = clear_level(100, 100);
        let mut rand = seeded();
        // y=Itof(97), vel_y=Itof(1): y -> Itof(98) (kIy=98), over air -> gravity makes
        // vel_y = 65536+1000 = 66536; kInewY = Ftoi(Itof(98)+66536) = 99 = height-1 ->
        // bounce. vel_y = -(66536*2)/3 = -44357 (|.|>=100 -> kept).
        let mut bonus = Bonus {
            x: itof(50),
            y: itof(97),
            vel_y: itof(1),
            frame: 0,
            timer: 1000,
            weapon: 0,
        };
        let (outcome, sobjects) = run_bonus_process(&mut bonus, true, &mut level, &mut rand, 1000, 2, 3);

        assert_eq!(outcome, BonusOutcome::Keep);
        assert_eq!(bonus.y, itof(98), "y += vel_y");
        assert_eq!(bonus.vel_y, -44357, "vel_y = -(post-gravity vel_y * Mul) / Div (truncating)");
        assert_eq!(rand.last(), 0, "the bounce path draws NO rand");
        assert_eq!(sobjects.len(), 0);
    }

    #[test]
    fn bounce_with_small_velocity_is_zeroed() {
        let mut level = clear_level(100, 100);
        let mut rand = seeded();
        // At the bottom row: kIy=99 so kIy+1 is OOB -> no gravity (vel_y stays 99);
        // kInewY = 99 = height-1 -> bounce. -(99*2)/3 = -66 -> |66| < 100 -> zeroed.
        let mut bonus = Bonus {
            x: itof(50),
            y: itof(99),
            vel_y: 99,
            frame: 0,
            timer: 1000,
            weapon: 0,
        };
        let (outcome, sobjects) = run_bonus_process(&mut bonus, true, &mut level, &mut rand, 1000, 2, 3);

        assert_eq!(outcome, BonusOutcome::Keep);
        assert_eq!(bonus.vel_y, 0, "|reflected| < 100 -> vel_y zeroed");
        assert_eq!(rand.last(), 0);
        assert_eq!(sobjects.len(), 0);
    }

    // ---- (c) expire: spawn sobject_types[bonus_s_objects[frame]] + free iff used --

    #[test]
    fn expire_spawns_sobject_and_frees_iff_used() {
        let mut level = clear_level(100, 100);

        // used = true -> Free; the flash sobject (index 7) spawns at (kIx-8, kIy-8)
        // and draws exactly one rand(3) (its sound).
        let mut rand = seeded();
        let mut bonus = Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 0,
            frame: 0,
            timer: 1,
            weapon: 0,
        };
        let (outcome, sobjects) = run_bonus_process(&mut bonus, true, &mut level, &mut rand, 1000, 2, 3);

        assert_eq!(outcome, BonusOutcome::Free, "--timer<=0 && used -> Free");
        assert_eq!(bonus.timer, 0, "--timer");
        assert_eq!(sobjects.len(), 1, "expiry spawned sobject_types[bonus_s_objects[0] == 7]");
        let s = *sobjects.get(0).expect("flash sobject in slot 0");
        assert_eq!(s.id, 7, "the flash sobject (id 7)");
        assert_eq!(s.x, 50 - 8, "sobject x = kIx - 8");
        assert_eq!(s.y, 50 - 8, "sobject y = kIy - 8");
        let mut refr = seeded();
        refr.bound(3); // the flash's single sound rand
        assert_eq!(rand.last(), refr.last(), "expiry drew exactly the flash sound rand(3)");

        // used = false -> Keep (slot NOT freed), but the sobject STILL spawns and the
        // SAME single rand is drawn: the `used` gate is on the FREE, not the spawn.
        let mut rand2 = seeded();
        let mut bonus2 = Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 0,
            frame: 0,
            timer: 1,
            weapon: 0,
        };
        let (outcome2, sobjects2) =
            run_bonus_process(&mut bonus2, false, &mut level, &mut rand2, 1000, 2, 3);

        assert_eq!(outcome2, BonusOutcome::Keep, "--timer<=0 but !used -> Keep (slot kept)");
        assert_eq!(sobjects2.len(), 1, "the expiry sobject spawns regardless of `used`");
        assert_eq!(rand2.last(), refr.last(), "same single flash rand whether or not used");
    }

    // ---- (d) free-during-iteration over the bonuses pool --------------------

    #[test]
    fn process_bonuses_frees_middle_slot_and_keeps_iterating() {
        // Slots 0,1,2. The MIDDLE (slot 1) expires this tick (timer=1) and is freed;
        // the outer two (timer high) survive AND must still be visited (timers
        // decrement) -> proves the slot-walk keeps iterating past a freed middle slot.
        let mut level = clear_level(100, 100);
        let mut rand = seeded();
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 0,
            frame: 0,
            timer: 1000,
            weapon: 0,
        });
        bonuses.spawn(Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 0,
            frame: 0,
            timer: 1, // expires
            weapon: 0,
        });
        bonuses.spawn(Bonus {
            x: itof(50),
            y: itof(50),
            vel_y: 0,
            frame: 0,
            timer: 1000,
            weapon: 0,
        });

        let cossin = precompute_cossin();
        let nts: Vec<NObjectType> = Vec::new();
        let sts = sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut worms: Vec<WormState> = Vec::new();
        let weps = weapons(5);
        process_bonuses(
            &mut bonuses,
            &mut level,
            &mut worms,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            &weps,
            &nts,
            &sts,
            &cossin,
            &SpriteSet::default(),
            &[],
            100,
            1000,
            2,
            3,
            &[7, 7],
            &mut rand,
        );

        assert_eq!(bonuses.len(), 2, "the middle (expired) bonus was freed");
        assert!(bonuses.get(1).is_none(), "slot 1 freed");
        let b0 = *bonuses.get(0).expect("slot 0 survives");
        let b2 = *bonuses.get(2).expect("slot 2 survives");
        assert_eq!(b0.timer, 999, "slot 0 was processed (--timer)");
        assert_eq!(b2.timer, 999, "slot 2 still iterated AFTER the freed middle slot (--timer)");
        assert_eq!(sobjects.len(), 1, "only the expired middle bonus spawned its expiry sobject");
    }

    // ---- Slice 5'b T8: bonus pickup (worm.cpp:287-322) ----------------------
    // The visible-worm pickup block. RNG contract (verified against :287-322):
    //   for each bonus in pool (slot) order, iff the 11x11 AABB gate passes:
    //     frame == 1 (health): iff health < settings_health -> Free + ONE
    //         rand(BonusHealthVar); DoHealingDirect((rand + BonusMinHealth) *
    //         settings_health / 100). health >= settings_health -> NO free, NO rand.
    //     frame == 0 (weapon): ALWAYS rand(BonusExplodeRisk).
    //         > 1  -> reload (ww.type/ammo unless HBonusReloadOnly, fire_cone=0),
    //                 Free, loading_left=0, no further rand.
    //         <= 1 -> booby: sobject_types[0].Create (its own draws) + Free.
    // Tests drive the standalone `worm_pickup_bonuses` against a SEEDED Rand and
    // replay a reference stream to pin the exact per-branch draw ORDER + COUNT.

    use crate::state::{WeaponInit, WormInit, NUM_WEAPONS};

    // A visible worm at pixel (px, py) with `health`, current_weapon 0.
    fn pickup_worm(px: i32, py: i32, health: i32) -> WormState {
        let init = WormInit {
            index: 0,
            health,
            lives: 3,
            stats_x: 0,
            weapons: [WeaponInit::default(); NUM_WEAPONS],
            start_pos: Vec2::new(itof(px), itof(py)),
            visible: true,
        };
        WormState::from_init(&init)
    }

    // Weapon defs where `weapons[i].id == i` and `ammo == 10 + i` — so a reload
    // from bonus.weapon = k is witnessable (ty -> Some(k), ammo -> 10 + k).
    fn weapon_defs() -> Vec<Weapon> {
        (0..5)
            .map(|i| Weapon {
                id: i,
                ammo: 10 + i,
                ..Default::default()
            })
            .collect()
    }

    // The booby sobject (sobject_types[0]). Tuned like the flash to draw EXACTLY
    // one rand — the sound `rand(num_sounds)` — and nothing else (damage = 0 skips
    // the worm/blow-away block, dirt_effect = -1 skips the carve). Spawns one
    // SObject (proof Create ran).
    fn booby_sobject_types() -> Vec<SObjectType> {
        let booby = SObjectType {
            id: 0,
            start_sound: 0,
            num_sounds: 3,
            anim_delay: 2,
            start_frame: 0,
            num_frames: 5,
            detect_range: 0,
            damage: 0,
            blow_away: 0,
            dirt_effect: -1,
            ..Default::default()
        };
        vec![booby]
    }

    // A health/weapon bonus at pixel (px, py).
    fn bonus_at(px: i32, py: i32, frame: i32, weapon: i32) -> Bonus {
        Bonus {
            x: itof(px),
            y: itof(py),
            timer: 500,
            weapon,
            frame,
            vel_y: 0,
        }
    }

    // Drive `worm_pickup_bonuses` with the common test plumbing. Returns the number
    // of SObjects the booby branch spawned (0 for health/reload/out-of-range).
    #[allow(clippy::too_many_arguments)]
    fn run_pickup(
        worms: &mut [WormState],
        wi: usize,
        bonuses: &mut Pool<Bonus>,
        rand: &mut Rand,
        weps: &[Weapon],
        settings_health: i32,
        health_var: i32,
        min_health: i32,
        explode_risk: i32,
        reload_only: bool,
    ) -> usize {
        let cossin = precompute_cossin();
        let nts: Vec<NObjectType> = Vec::new();
        let sts = booby_sobject_types();
        let (mut wobjects, mut nobjects, mut sobjects) = empty_pools();
        let mut level = clear_level(300, 300);
        worm_pickup_bonuses(
            worms,
            wi,
            bonuses,
            &mut wobjects,
            &mut nobjects,
            &mut sobjects,
            weps,
            &nts,
            &sts,
            &mut level,
            &cossin,
            &SpriteSet::default(),
            &[],
            0,
            settings_health,
            health_var,
            min_health,
            explode_risk,
            reload_only,
            rand,
        );
        sobjects.len()
    }

    // ---- (a) the 11x11 AABB gate rejects an out-of-range bonus --------------

    #[test]
    fn out_of_range_bonus_draws_no_rand_and_is_not_freed() {
        let mut worms = vec![pickup_worm(100, 100, 50)];
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        // |dx| = 100 > 5 -> outside the box on the x sides.
        bonuses.spawn(bonus_at(200, 200, 1, 0));
        let mut rand = seeded();

        let before = rand.draws();
        let spawned = run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &weapons(5), 100, 20, 30, 1000, false,
        );

        assert_eq!(rand.draws(), before, "out-of-range bonus draws NO rand");
        assert_eq!(bonuses.len(), 1, "out-of-range bonus not freed");
        assert_eq!(worms[0].health, 50, "health untouched");
        assert_eq!(spawned, 0, "no sobject");
    }

    // ---- (b) health bonus, health < settings_health: ONE draw, heal, free ---

    #[test]
    fn health_bonus_below_max_heals_and_frees_with_one_draw() {
        let settings_health = 100;
        let health_var = 20;
        let min_health = 30;
        let start = 10;
        let mut worms = vec![pickup_worm(100, 100, start)];
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(100, 100, 1, 0));
        let mut rand = seeded();

        // Reference: exactly one rand(BonusHealthVar); amount is
        // (rand + BonusMinHealth) * settings_health / 100 (truncating /).
        let mut refr = seeded();
        let ex_amount =
            (refr.bound(health_var as u32) as i32 + min_health) * settings_health / 100;
        let ex_health = (start + ex_amount).min(settings_health);

        let before = rand.draws();
        run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &weapons(5), settings_health, health_var,
            min_health, 1000, false,
        );

        assert_eq!(rand.draws() - before, 1, "health branch draws rand(BonusHealthVar) once");
        assert_eq!(rand.last(), refr.last(), "the single draw is rand(BonusHealthVar)");
        assert_eq!(worms[0].health, ex_health, "DoHealingDirect: health += amount, clamped");
        assert_eq!(bonuses.len(), 0, "the health bonus was freed");
    }

    // ---- (c) health bonus, health >= settings_health: NO free, NO rand ------

    #[test]
    fn health_bonus_at_full_health_draws_nothing_and_keeps_bonus() {
        let settings_health = 100;
        let mut worms = vec![pickup_worm(100, 100, settings_health)];
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(100, 100, 1, 0));
        let mut rand = seeded();

        let before = rand.draws();
        run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &weapons(5), settings_health, 20, 30, 1000,
            false,
        );

        assert_eq!(rand.draws(), before, "health >= settings_health: NO rand");
        assert_eq!(bonuses.len(), 1, "full-health worm does NOT free the bonus");
        assert_eq!(worms[0].health, settings_health, "health unchanged");
    }

    // ---- (d) weapon bonus, rand(BonusExplodeRisk) > 1: reload, ONE draw -----

    #[test]
    fn weapon_bonus_reload_sets_type_ammo_clears_cones_and_frees_one_draw() {
        let explode_risk = 1000; // large -> rand(risk) > 1 (guarded)
        let mut refr = seeded();
        let ex_roll = refr.bound(explode_risk as u32);
        assert!(ex_roll > 1, "seed guard: this seed must land the reload (> 1) branch");

        let defs = weapon_defs(); // weapon 3 -> id 3, ammo 13
        let mut worms = vec![pickup_worm(100, 100, 100)];
        worms[0].current_weapon = 0;
        worms[0].fire_cone = 9;
        worms[0].weapons[0].loading_left = 5;
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(100, 100, 0, 3));
        let mut rand = seeded();

        let before = rand.draws();
        let spawned = run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &defs, 100, 20, 30, explode_risk, false,
        );

        assert_eq!(rand.draws() - before, 1, "reload draws only rand(BonusExplodeRisk)");
        assert_eq!(rand.last(), refr.last(), "the single draw is rand(BonusExplodeRisk)");
        assert_eq!(worms[0].weapons[0].ty, Some(3), "ww.type = &weapons[bonus.weapon] (hashed id)");
        assert_eq!(worms[0].weapons[0].ammo, 13, "ww.ammo = ww.type->ammo");
        assert_eq!(worms[0].fire_cone, 0, "fire_cone = 0 on reload");
        assert_eq!(worms[0].weapons[0].loading_left, 0, "loading_left = 0");
        assert_eq!(bonuses.len(), 0, "weapon bonus freed");
        assert_eq!(spawned, 0, "reload branch spawns NO sobject");
    }

    // ---- (e) HBonusReloadOnly: reload clears loading_left ONLY --------------

    #[test]
    fn weapon_bonus_reload_only_hack_clears_loading_left_only() {
        let explode_risk = 1000;
        let mut refr = seeded();
        assert!(refr.bound(explode_risk as u32) > 1, "seed guard: reload branch");

        let defs = weapon_defs();
        let mut worms = vec![pickup_worm(100, 100, 100)];
        worms[0].fire_cone = 9;
        worms[0].weapons[0].loading_left = 5;
        worms[0].weapons[0].ty = Some(1);
        worms[0].weapons[0].ammo = 99;
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(100, 100, 0, 3));
        let mut rand = seeded();

        run_pickup(&mut worms, 0, &mut bonuses, &mut rand, &defs, 100, 20, 30, explode_risk, true);

        assert_eq!(worms[0].weapons[0].ty, Some(1), "HBonusReloadOnly: type NOT swapped");
        assert_eq!(worms[0].weapons[0].ammo, 99, "HBonusReloadOnly: ammo NOT refilled");
        assert_eq!(worms[0].fire_cone, 9, "HBonusReloadOnly: fire_cone NOT cleared");
        assert_eq!(worms[0].weapons[0].loading_left, 0, "loading_left still cleared");
        assert_eq!(bonuses.len(), 0, "bonus still freed");
    }

    // ---- (f) weapon bonus, rand(BonusExplodeRisk) <= 1: booby spawn ---------

    #[test]
    fn weapon_bonus_booby_spawns_sobject_and_frees() {
        // rand(2) is always in {0,1} -> always <= 1 -> booby (seed-independent).
        let explode_risk = 2;
        let defs = weapon_defs();
        let mut worms = vec![pickup_worm(100, 100, 100)];
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(103, 97, 0, 3)); // dx=3, dy=-3 -> inside the 11x11 box
        let mut rand = seeded();

        // Reference: rand(2) [the risk roll], then the booby sobject's ONE sound
        // rand(3).
        let mut refr = seeded();
        assert!(refr.bound(2) <= 1, "rand(2) is always <= 1");
        refr.bound(3); // booby sobject_types[0] sound
        let expected_last = refr.last();

        let before = rand.draws();
        let spawned = run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &defs, 100, 20, 30, explode_risk, false,
        );

        assert_eq!(rand.draws() - before, 2, "booby: rand(BonusExplodeRisk) + the sobject sound");
        assert_eq!(rand.last(), expected_last, "order: rand(risk) then the booby sobject cluster");
        assert_eq!(spawned, 1, "the booby sobject_types[0] was created");
        assert_eq!(bonuses.len(), 0, "the weapon bonus was freed");
        assert_eq!(worms[0].weapons[0].ty, None, "booby branch does not reload");
    }

    // ---- (g) multiple bonuses iterated in pool (slot) order -----------------

    #[test]
    fn multiple_health_bonuses_iterate_in_slot_order() {
        let settings_health = 100;
        let health_var = 20;
        let min_health = 5;
        let mut worms = vec![pickup_worm(100, 100, 10)];
        let mut bonuses: Pool<Bonus> = Pool::new(99);
        bonuses.spawn(bonus_at(100, 100, 1, 0)); // slot 0
        bonuses.spawn(bonus_at(102, 98, 1, 0)); // slot 1
        let mut rand = seeded();

        // Reference: slot 0 heals on the running health, then slot 1 heals on the
        // updated health — two rand(BonusHealthVar) in slot order.
        let mut refr = seeded();
        let a0 = (refr.bound(health_var as u32) as i32 + min_health) * settings_health / 100;
        let h1 = (10 + a0).min(settings_health);
        let a1 = (refr.bound(health_var as u32) as i32 + min_health) * settings_health / 100;
        let h2 = (h1 + a1).min(settings_health);

        let before = rand.draws();
        run_pickup(
            &mut worms, 0, &mut bonuses, &mut rand, &weapons(5), settings_health, health_var,
            min_health, 1000, false,
        );

        assert_eq!(rand.draws() - before, 2, "each health bonus draws once, in slot order");
        assert_eq!(worms[0].health, h2, "both heals applied on the running health");
        assert_eq!(bonuses.len(), 0, "both bonuses freed");
    }
}
