//! Step 4½e-1 — C++ `WeaponMenuState` (`weaponMenuState.cpp`; design §4.4, plan facts 10-11).
//! Batch 4 (T3) lands the screen's slot on the stack; Batch 5 (T4) ports the state: its 40-row
//! menu, `update`'s key order, the close rule and `draw`.

/// The `WeaponMenuState` screen. A placeholder until T4 fills it in: nothing pushes it yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponMenuState {}
