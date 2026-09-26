//! The preview's `?weapons=` loadout (moved from `game::web_params`, Step 4½d): NEW GAME re-applies it to every fresh tick-0 state.

use sim::state::{NUM_WEAPONS, SimState, WormWeapon};

/// Rewrite both worms' tick-0 loadout: slot `i` gets `names[i]` (matched against
/// the TC weapon table, case-insensitively) with that weapon's starting ammo,
/// exactly as `Worm::InitWeapons` would. Slots beyond `names` keep their default.
/// Returns the names that matched no weapon (their slots are left unchanged).
pub fn apply_weapons(state: &mut SimState, names: &[String]) -> Vec<String> {
    let mut unknown = Vec::new();
    for (slot, name) in names.iter().enumerate().take(NUM_WEAPONS) {
        let Some(id) = state
            .weapons
            .iter()
            .position(|w| w.name.eq_ignore_ascii_case(name))
        else {
            unknown.push(name.clone());
            continue;
        };
        let ammo = state.weapons[id].ammo;
        for worm in &mut state.worms {
            worm.weapons[slot] = WormWeapon {
                ty: Some(id as _),
                ammo,
                delay_left: 0,
                loading_left: 0,
            };
        }
    }
    unknown
}
