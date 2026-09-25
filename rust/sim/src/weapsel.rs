//! Step 4½c — the weapon selection phase (`weapsel.cpp`), Bevy-free (overview LD 7; design
//! `specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md`, cited **design §N**).
//!
//! Every C++ match starts with a pick screen whose constructor and RANDOMIZE item draw from
//! `game.rand`, the SIMULATION RNG (`weapsel.cpp:61`, `:68`, `:323`), so the number and order of
//! draws before match frame 0 decides every later tick. The phase therefore lives next to
//! `SimState`: it draws `state.rand` and writes the worms' control words and weapons (both hashed,
//! `hash.rs:49`, `:65-73`), and Step 5 snapshots it as `SimState` plus a `WeaponSelection` clone
//! (design §8). It is driven only from `tick_and_render` (LD 3); `game` owns the presentation.

use assets::object::Weapon;

/// `Common::weap_order` (`common.cpp:491-499`): weapon indices sorted by name. A saved pick `p`
/// (1-based, `WormSettings::weapons`) names weapon `weap_order[p - 1]`. The one shared copy
/// (design §4.7). C++ sorts unstably and Rust stably; the two agree while the TC's weapon names
/// are unique, which `the_tc_weapon_names_are_unique_so_both_sorts_agree` pins (finding 12).
pub fn weap_order(weapons: &[Weapon]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..weapons.len()).collect();
    order.sort_by(|&a, &b| weapons[a].name.cmp(&weapons[b].name));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc_weapons() -> Vec<Weapon> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc =
            assets::tc::TcConfig::load(&std::fs::read(format!("{root}/tc.cfg")).unwrap()).unwrap();
        assets::object::Objects::load(&tc.types, |sub, id| {
            std::fs::read(format!("{root}/{sub}/{id}.cfg"))
        })
        .unwrap()
        .weapons
    }

    #[test]
    fn the_tc_weapon_names_are_unique_so_both_sorts_agree() {
        let weapons = tc_weapons();
        let mut names: Vec<&str> = weapons.iter().map(|w| w.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            weapons.len(),
            "finding 12: C++ sorts weap_order unstably (common.cpp:498-499), Rust stably — \
             they agree only while weapon names are unique"
        );
    }

    #[test]
    fn weap_order_is_the_byte_order_of_the_names() {
        let weapons = tc_weapons();
        let order = weap_order(&weapons);
        assert_eq!(order.len(), 40);
        let name = |pick: usize| weapons[order[pick - 1]].name.as_str();
        // The picks the 4½c corpus and the live default loadout rely on (T6, T9).
        assert_eq!(
            (name(1), name(12), name(26), name(31), name(40)),
            ("BAZOOKA", "DART", "LASER", "MISSILE", "ZIMM")
        );
        // std::string's `<` is a byte compare: the space (0x20) sorts before letters.
        assert_eq!(
            (name(28), name(29), name(30)),
            ("MINI NUKE", "MINI ROCKETS", "MINIGUN")
        );
    }
}
