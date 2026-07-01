//! Port of `Game::CreateBObject` + `BObject::Process` (`bobject.cpp:7-49`) — the
//! blood-pool spawn and per-tick driver.
//!
//! Blood **nobjects** (type 6) spray these via their blood-trail
//! (`nobject.cpp:95-97` -> [`create_bobject`]); each `BObject` then falls under
//! gravity and, on landing, paints one level pixel and frees itself. The pool is a
//! `FastObjectList` (the [`BloodPool`]), so the driver's swap-remove order is the
//! whole hash contract (the fold reads `pos.x`/`pos.y` only — see
//! [`crate::state::BObject`]).
//!
//! ## RNG order is the contract
//!
//! Two draws advance the shared stream:
//! * [`create_bobject`] draws **one** `rand(NumBloodColours)` (`:12`) for the
//!   colour. The colour is never hashed, but the draw is load-bearing.
//! * [`bobject_process`] draws **at most one** `rand(3)` (`:36`/`:40`/`:44`), only
//!   on the landing tick, in the chosen one of three mutually exclusive branches.
//!   Off-map and the in-air "stay" path draw nothing.

use sim_core::fixed::ftoi;
use sim_core::rng::Rand;
use sim_core::vec::Vec2;

use crate::pool::BloodPool;
use crate::state::{BObject, LevelSim};

/// Port of `Game::CreateBObject` (`bobject.cpp:7-15`).
///
/// Rolls the (render-only) blood colour — `rand(num_blood_colours) +
/// first_blood_colour` — then appends one [`BObject`] carrying `pos`/`vel` into the
/// blood pool. The colour value is **not** hashed, but the `rand(num_blood_colours)`
/// DRAW advances the shared RNG and is part of the contract (so it runs even though
/// the result is only stored, never read by [`bobject_process`]).
///
/// The blood-trail caller passes the spraying nobject's `pos` and `vel / 4`
/// (`nobject.cpp:96`). A full pool (the C++ `NewObjectReuse` slot-0 reuse, O3) is
/// deferred: under the faithful 1/10 blood-trail cadence the pool stays well under
/// its 700 cap, so a `None` spawn (drop) is unreachable here.
pub fn create_bobject(
    bobjects: &mut BloodPool<BObject>,
    pos: Vec2,
    vel: Vec2,
    num_blood_colours: i32,
    first_blood_colour: i32,
    rand: &mut Rand,
) {
    // :12 color = rand(NumBloodColours) + FirstBloodColour. Load-bearing DRAW;
    // colour value render-only (not hashed).
    let color = rand.bound(num_blood_colours as u32) as i32 + first_blood_colour;
    // :10-14 NewObjectReuse, then write pos/vel. Pool-full reuse deferred (O3).
    let _ = bobjects.spawn(BObject { pos, vel, color });
}

/// Port of `BObject::Process` (`bobject.cpp:17-49`) — advance one blood particle by
/// one tick. Returns `true` to stay alive, `false` for the driver to free it (the
/// C++ `if (i->Process(*this)) ++i; else Free(i)`, `game.cpp:349-354`).
///
/// 1. `pos += vel` (`:20`).
/// 2. off-map (`!Inside`) -> `false`, **no rand** (`:24`).
/// 3. read the OLD pixel colour `c` + material (`:27-28`, before any write).
/// 4. background air -> `vel.y += BObjGravity` (`:30-32`); no free (falls on).
/// 5. landing bands (mutually exclusive, each writes one pixel and frees, drawing
///    exactly one `rand(3)`):
///    * `c in 1..=2 || c in 77..=79` -> `SetPixel(77 + rand(3))` (`:34-38`);
///    * else `AnyDirt` -> `SetPixel(82 + rand(3))` (`:39-42`);
///    * else `Rock` -> `SetPixel(85 + rand(3))` (`:43-46`).
/// 6. otherwise `true` — stays alive, **no rand** (`:48`).
///
/// `SetPixel` reuses the [`LevelSim::set_material`] write path (the same one
/// `draw_dirt_effect` uses): in this port `material_id` IS the C++ pixel buffer, so
/// writing it is exactly `SetPixel`'s `material_id[idx] = w` and the subsequent
/// material reads pick up the new flag byte via `material_flags`. The write updates
/// the HASHED `level` component.
pub fn bobject_process(
    obj: &mut BObject,
    level: &mut LevelSim,
    bobj_gravity: i32,
    rand: &mut Rand,
) -> bool {
    // :20 pos += vel.
    obj.pos = obj.pos.add(obj.vel);

    let ipos_x = ftoi(obj.pos.x);
    let ipos_y = ftoi(obj.pos.y);

    // :24 off-map -> free, NO rand.
    if !level.inside(ipos_x, ipos_y) {
        return false;
    }

    // :27-28 read the OLD pixel colour + material BEFORE any SetPixel.
    let c = level.pixel(ipos_x, ipos_y);
    let idx = (ipos_x + ipos_y * level.width) as usize;

    // :30-32 background air -> gravity (no free; keeps falling).
    if level.background(ipos_x, ipos_y) {
        obj.vel.y = obj.vel.y.wrapping_add(bobj_gravity);
    }

    // :34-38 blood-on-blood / pixel band 1..=2 or 77..=79 -> paint + free (1 rand).
    if (1..=2).contains(&c) || (77..=79).contains(&c) {
        level.set_material(idx, (77 + rand.bound(3) as i32) as u8);
        return false;
    }
    // :39-42 dirt -> paint + free (1 rand).
    if level.any_dirt(ipos_x, ipos_y) {
        level.set_material(idx, (82 + rand.bound(3) as i32) as u8);
        return false;
    }
    // :43-46 rock -> paint + free (1 rand).
    if level.rock(ipos_x, ipos_y) {
        level.set_material(idx, (85 + rand.bound(3) as i32) as u8);
        return false;
    }

    // :48 stays alive (no rand).
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MAT_BACKGROUND, MAT_DIRT, MAT_ROCK};
    use sim_core::fixed::itof;

    const SEED: u32 = 0x9E3D;

    fn seeded() -> Rand {
        let mut r = Rand::new();
        r.seed(SEED);
        r
    }

    // A 1-pixel-material level: every pixel carries material id `mat_id`, whose flag
    // byte is `flag`. `pixel(x,y)` therefore returns `mat_id` and the material probes
    // read `flag`. Width/height 100 so coordinates are comfortably in range.
    fn uniform_level(mat_id: u8, flag: u8) -> LevelSim {
        let mut material_flags = [0u8; 256];
        material_flags[mat_id as usize] = flag;
        LevelSim {
            width: 100,
            height: 100,
            material_id: vec![mat_id; 100 * 100],
            material_flags,
        }
    }

    // ---- create_bobject: exactly one colour draw, pos/vel carried ------------

    #[test]
    fn create_bobject_draws_one_color_rand_and_carries_pos_vel() {
        let mut bobjects: BloodPool<BObject> = BloodPool::new(700);
        let mut rand = seeded();

        // Reference: a single rand(NumBloodColours) draw.
        let mut refr = seeded();
        let color_draw = refr.bound(9) as i32; // NumBloodColours = 9

        let pos = Vec2::new(itof(40), itof(30));
        let vel = Vec2::new(itof(8), itof(-12));
        create_bobject(&mut bobjects, pos, vel, 9, 64, &mut rand);

        assert_eq!(bobjects.len(), 1, "exactly one bobject spawned");
        let b = *bobjects.iter().next().unwrap();
        assert_eq!(b.pos, pos, "bobject.pos = the nobject's pos");
        assert_eq!(b.vel, vel, "bobject.vel = the supplied (vel/4) vector");
        assert_eq!(
            b.color,
            color_draw + 64,
            "color = rand(NumBloodColours) + FirstBloodColour"
        );
        assert_eq!(
            rand.last(),
            refr.last(),
            "CreateBObject draws EXACTLY one rand (the colour roll)"
        );
    }

    #[test]
    fn create_bobject_blood_trail_uses_vel_div_4_truncating() {
        // The blood-trail arm passes vel/4 (truncating). Pin that the value carried
        // is the divided vector, not the raw vel.
        let mut bobjects: BloodPool<BObject> = BloodPool::new(8);
        let mut rand = seeded();
        let nob_vel = Vec2::new(itof(9), itof(-9));
        let quarter = nob_vel.div(4);
        create_bobject(&mut bobjects, Vec2::zero(), quarter, 4, 0, &mut rand);
        let b = *bobjects.iter().next().unwrap();
        assert_eq!(b.vel, quarter, "carries vel/4");
        assert_ne!(b.vel, nob_vel, "not the undivided vel");
    }

    // ---- bobject_process: background -> gravity, stays, no rand --------------

    #[test]
    fn process_background_applies_gravity_and_stays_without_rand() {
        let mut level = uniform_level(0, MAT_BACKGROUND); // colour 0, background flag
        let mut rand = seeded();
        rand.bound(123); // pre-advance so "no rand" is a real assertion
        let rng_before = rand.last();

        let mut obj = BObject {
            pos: Vec2::new(itof(40), itof(40)),
            vel: Vec2::new(itof(1), itof(2)),
            color: 0,
        };
        let level_before = level.material_id.clone();

        let alive = bobject_process(&mut obj, &mut level, 700, &mut rand);

        assert!(alive, "background air -> particle stays alive");
        assert_eq!(obj.pos, Vec2::new(itof(41), itof(42)), "pos += vel");
        assert_eq!(
            obj.vel,
            Vec2::new(itof(1), itof(2) + 700),
            "vel.y += BObjGravity in background air"
        );
        assert_eq!(rand.last(), rng_before, "background path draws NO rand");
        assert_eq!(level.material_id, level_before, "no SetPixel in air");
    }

    // ---- bobject_process: off-map -> free, no rand --------------------------

    #[test]
    fn process_off_map_frees_without_rand() {
        let mut level = uniform_level(0, MAT_BACKGROUND);
        let mut rand = seeded();
        rand.bound(7);
        let rng_before = rand.last();

        // vel carries it off the left edge: pos.x + vel.x -> negative -> !Inside.
        let mut obj = BObject {
            pos: Vec2::new(itof(0), itof(40)),
            vel: Vec2::new(itof(-5), 0),
            color: 0,
        };

        let alive = bobject_process(&mut obj, &mut level, 700, &mut rand);

        assert!(!alive, "off-map -> free");
        assert_eq!(rand.last(), rng_before, "off-map draws NO rand");
    }

    // ---- bobject_process: the three landing bands each paint + free, 1 rand --

    // Helper: land on a level whose every pixel is (colour `c`, material `flag`), and
    // assert exactly one rand(3) was drawn, the verdict is free, and the painted
    // colour is `base + that draw`.
    fn assert_landing(mat_id: u8, flag: u8, base: i32, why: &str) {
        let mut level = uniform_level(mat_id, flag);
        let mut rand = seeded();
        // Reference draw for the painted colour.
        let mut refr = seeded();
        let d = refr.bound(3) as i32;

        // Zero vel so pos stays put and ipos = (40,40), an in-range landing cell.
        let mut obj = BObject {
            pos: Vec2::new(itof(40), itof(40)),
            vel: Vec2::zero(),
            color: 0,
        };
        let idx = 40 + 40 * level.width;

        let alive = bobject_process(&mut obj, &mut level, 700, &mut rand);

        assert!(!alive, "{why}: landing -> free");
        assert_eq!(
            level.material_id[idx as usize],
            (base + d) as u8,
            "{why}: SetPixel(base + rand(3))"
        );
        assert_eq!(
            rand.last(),
            refr.last(),
            "{why}: exactly one rand(3) on the landing tick"
        );
    }

    #[test]
    fn process_pixel_band_paints_77_plus_rand3() {
        // colour 1 is in the 1..=2 band; flag has no background/dirt/rock bit so only
        // the pixel-band branch fires.
        assert_landing(1, 0, 77, "pixel band 1..=2");
        // colour 78 is in the 77..=79 band.
        assert_landing(78, 0, 77, "pixel band 77..=79");
    }

    #[test]
    fn process_dirt_paints_82_plus_rand3() {
        // colour 200 (outside the pixel bands) with the dirt flag -> AnyDirt branch.
        assert_landing(200, MAT_DIRT, 82, "dirt");
    }

    #[test]
    fn process_rock_paints_85_plus_rand3() {
        // colour 201 (outside the pixel bands, not dirt) with the rock flag.
        assert_landing(201, MAT_ROCK, 85, "rock");
    }

    // ---- driver loop: swap-remove order pinned when a middle slot frees ------

    #[test]
    fn driver_loop_frees_landed_and_pins_swap_remove_order() {
        // Five particles in a level that is background for x < 50 and rock for
        // x >= 50. Particles that land on rock free (Process false); the others stay.
        // Freeing a middle slot swap-removes the last live particle into it, so the
        // surviving order is NOT the original. The two landers (B@50, D@60) sit on
        // DISTINCT cells so one's SetPixel can't mask the other's material probe.
        let mut level = uniform_level(0, MAT_BACKGROUND);
        for y in 0..level.height {
            for x in 50..level.width {
                level.material_id[(x + y * level.width) as usize] = 201; // rock colour
            }
        }
        level.material_flags[201] = MAT_ROCK;

        let mut rand = seeded();
        let mut bobjects: BloodPool<BObject> = BloodPool::new(8);
        // Tag colour as an identity so we can read the survivor order. vel zero so
        // pos is unchanged: particles at x>=50 land (rock), others stay (background).
        let mk = |x: i32, id: i32| BObject {
            pos: Vec2::new(itof(x), itof(40)),
            vel: Vec2::zero(),
            color: id,
        };
        // slots: [A@10 stay, B@50 land, C@20 stay, D@60 land, E@30 stay]
        for b in [mk(10, 1), mk(50, 2), mk(20, 3), mk(60, 4), mk(30, 5)] {
            bobjects.spawn(b);
        }

        bobjects.retain_processing(|obj| bobject_process(obj, &mut level, 700, &mut rand));

        // Swap-remove trace (free == Process false at x=50):
        //   [A,B,C,D,E] i=1 free B -> [A,E,C,D]  (E into slot 1)
        //   [A,E,C,D]   i=3 free D -> [A,E,C]
        let order: Vec<i32> = bobjects.iter().map(|b| b.color).collect();
        assert_eq!(
            order,
            vec![1, 5, 3],
            "swap-remove: B freed (E jumps in), D freed -> survivors A,E,C"
        );
        assert_eq!(bobjects.len(), 3, "two landed particles freed");
    }
}
