//! Heltals-kvadratrot och vektorlängd. Port av Sqr/VectorLength i src/game/math.cpp.

/// Heltals-sqrt (avrundat nedåt). Bit-för-bit-port av Sqr().
pub fn isqrt(mut op: u32) -> u32 {
    let mut res: u32 = 0;
    let mut one: u32 = 1 << 30; // högsta fyrpotens
    while one > op {
        one >>= 2;
    }
    while one != 0 {
        if op >= res + one {
            op -= res + one;
            res += 2 * one;
        }
        res >>= 1;
        one >>= 2;
    }
    res
}

/// Port av VectorLength: isqrt(x*x + y*y), i32-aritmetik som castas till u32.
pub fn vector_length(x: i32, y: i32) -> i32 {
    let sum = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
    isqrt(sum as u32) as i32
}
