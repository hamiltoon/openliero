//! cossin_table[128], port av PrecomputeTables i src/game/math.cpp.
//! Egen heltals-Taylorserie (i64) — ingen libm, fullt reproducerbar.
use crate::vec::Vec2;

struct Fp {
    s: i64,
    bits: i32,
}

impl Fp {
    fn reduce(&mut self, tobits: i32) {
        let lim: i64 = 1i64 << tobits;
        while self.s < (-lim - 1) || self.s > lim {
            self.s >>= 1;
            self.bits -= 1;
        }
    }
    fn reducedfrac(&self, tobits: i32) -> i64 {
        let mut rs = self.s;
        let mut rbits = self.bits;
        while rbits > 60 {
            rs >>= 1;
            rbits -= 1;
        }
        rs << (tobits - rbits)
    }
}

pub fn precompute_cossin() -> [Vec2; 128] {
    const SCALE_BITS: i32 = 28;
    const SCALE: i32 = 13176795; // (2pi / 128) << scalebits
    let mut table = [Vec2::zero(); 128];
    for i in 0..128i32 {
        let mut rf: i64 = 0;
        let mut c: i32 = -1;
        let xf: i32 = i * SCALE;
        let mut num = Fp {
            s: xf as i64,
            bits: SCALE_BITS,
        };
        let mut t: i32 = 1;
        while t < 26 {
            rf = rf.wrapping_add((c as i64).wrapping_mul(num.reducedfrac(60)));

            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            c = -c;
        }
        const SHIFT: i32 = 60 - 16;
        rf += 1i64 << (SHIFT - 1); // korrekt avrundning
        let r = (rf >> SHIFT) as i32;
        table[i as usize].x = r;
        table[((i + 32) & 0x7f) as usize].y = r;
    }
    table
}
