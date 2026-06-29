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
use sim_core::fixed::itof;
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
                blood,
                rand,
            );
            return;
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
}
