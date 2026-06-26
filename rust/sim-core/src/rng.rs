//! Deterministisk MT19937, port av std::mt19937-användningen i src/game/rand.hpp.
//! Standard-parametrar; single-seed-init matchar C++:s std::mt19937(seed).

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

pub struct Rand {
    mt: [u32; N],
    idx: usize,
    #[allow(dead_code)]
    last: u32,
}

impl Rand {
    pub fn new() -> Self {
        let mut r = Rand {
            mt: [0; N],
            idx: N + 1,
            last: 0,
        };
        r.seed(0x1337);
        r
    }

    pub fn seed(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.idx = N;
        self.last = 0;
    }

    fn generate(&mut self) {
        for i in 0..N {
            let y = (self.mt[i] & UPPER_MASK) | (self.mt[(i + 1) % N] & LOWER_MASK);
            let mut next = self.mt[(i + M) % N] ^ (y >> 1);
            if y & 1 != 0 {
                next ^= MATRIX_A;
            }
            self.mt[i] = next;
        }
        self.idx = 0;
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.idx >= N {
            self.generate();
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        self.last = y;
        y
    }

    /// [0, max) via Lemire multiply-shift (matchar rand.hpp).
    pub fn bound(&mut self, max: u32) -> u32 {
        ((self.next_u32() as u64 * max as u64) >> 32) as u32
    }

    /// [min, max).
    pub fn bound_range(&mut self, min: u32, max: u32) -> u32 {
        self.bound(max - min) + min
    }
}

impl Default for Rand {
    fn default() -> Self {
        Self::new()
    }
}
