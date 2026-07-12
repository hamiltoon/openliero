//! Golden-vector tests against the C++ oracle. See tests/.
//!
//! The scenario parser + tick-0 loader now live in the Bevy-free `scenario`
//! crate (Step 3, Slice 3c, T0). This crate re-exports it so the existing
//! `oracle_tests::scenario::Scenario` references across the tests/examples
//! compile unchanged.

pub use scenario;
