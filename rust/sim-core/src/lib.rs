//! Deterministisk simuleringskärna för Liero-rs. Ingen Bevy-, std-rng- eller
//! flyttalsberoende — allt är heltalsaritmetik som matchar C++-motorn bit-exakt.

pub mod fixed;
pub mod vec;
