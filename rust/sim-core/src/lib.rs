//! Deterministisk simuleringskärna för Liero-rs. Ingen Bevy-, std-rng- eller
//! flyttalsberoende — allt är heltalsaritmetik som matchar C++-motorn bit-exakt.

pub mod fixed;
pub mod math;
pub mod rng;
pub mod tables;
pub mod vec;
