//! 16.16 fixpunkt. Port av src/game/math.hpp (Itof/Ftoi).
pub type Fixed = i32;
pub const FRAC_BITS: u32 = 16;

/// Heltal → fixpunkt: v << 16. Wrap matchar C++ 2-komplement.
#[inline]
pub fn itof(v: i32) -> Fixed {
    v.wrapping_shl(FRAC_BITS)
}

/// Fixpunkt → heltal: v >> 16 (aritmetiskt skift, som C++ signed >>).
#[inline]
pub fn ftoi(v: Fixed) -> i32 {
    v >> FRAC_BITS
}
