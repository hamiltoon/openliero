//! # Deterministic random number generator (MT19937)
//!
//! Port of `Rand` in `src/game/rand.hpp`, which internally is C++'s
//! `std::mt19937`.
//!
//! Randomness in Liero (weapon spread, splinter angles, crater appearance, …)
//! must be *reproducible*: both machines in a netplay match, and every replay
//! playback, must draw exactly the same "random" values in exactly the same
//! order — otherwise the worlds drift apart (desync). So no real randomness
//! (`std::random`/OS entropy) is used; instead a **seeded pseudo-random
//! generator** whose entire sequence is determined by the starting seed.
//!
//! The algorithm is **Mersenne Twister (MT19937)** — a standard, fully specified
//! generator. We implement it by hand (no `rand` crate) partly to keep
//! `sim-core` dependency-free, and partly for full control so it is
//! *bit-identical* to C++'s `std::mt19937`.

// Standard parameters for MT19937 (the 32-bit variant). Do not change anything
// here — they define the algorithm and must match C++ exactly.
const N: usize = 624; // state size (number of 32-bit words)
const M: usize = 397; // "twist" offset
const MATRIX_A: u32 = 0x9908_b0df; // twist matrix constant
const UPPER_MASK: u32 = 0x8000_0000; // the top bit
const LOWER_MASK: u32 = 0x7fff_ffff; // the lower 31 bits

/// The generator's state. `mt` is the 624 words, `idx` points at the next word
/// to read (when it reaches 624, `generate` regenerates the whole state).
pub struct Rand {
    mt: [u32; N],
    idx: usize,
    // Mirrors C++'s `last` (the most recently generated value). Exposed via
    // `last()` for state hashing and serialization parity with rand.hpp.
    last: u32,
    // Monotonic count of raw draws (`next_u32` calls) since construction. Pure
    // test/diagnostic instrumentation with NO C++ counterpart: it is NOT hashed
    // (neither the master nor the component fold reads it — they read `last()`
    // only), NOT serialized, and never influences the sequence. It lets the
    // differential harnesses witness a per-tick RNG *burst* (draws-per-tick)
    // directly from the driven state — e.g. the death-spray and the
    // BeginRespawn trial-count bursts of Slice 5d. Adding it leaves every
    // existing golden byte-identical.
    draws: u64,
}

impl Rand {
    /// Creates the generator with the game's default seed `0x1337` (same as
    /// rand.hpp).
    pub fn new() -> Self {
        let mut r = Rand {
            mt: [0; N],
            idx: N + 1,
            last: 0,
            draws: 0,
        };
        r.seed(0x1337);
        r
    }

    /// Initializes the whole state from a seed. This is MT19937's standard init
    /// (`init_genrand`): each word is derived from the previous one with a
    /// multiply and an xor mix. Same formula as C++'s `std::mt19937(seed)`, so
    /// the seed yields an identical sequence in both. `wrapping_*` because the
    /// multiply intentionally wraps around in 32 bits.
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

    /// The "twist": mix the whole state into the next generation of 624 words.
    /// Runs automatically once all words are consumed. This is the heart of
    /// MT19937.
    fn generate(&mut self) {
        for i in 0..N {
            // Combine the top bit of word i with the lower 31 bits of the next word.
            let y = (self.mt[i] & UPPER_MASK) | (self.mt[(i + 1) % N] & LOWER_MASK);
            let mut next = self.mt[(i + M) % N] ^ (y >> 1);
            if y & 1 != 0 {
                next ^= MATRIX_A; // conditional xor if the lowest bit is set
            }
            self.mt[i] = next;
        }
        self.idx = 0;
    }

    /// The next raw 32-bit random value. After the twist, each word is
    /// "tempered" with a fixed sequence of shifts and xors — this improves the
    /// statistical properties. The constants (`11, 7/0x9d2c5680, 15/0xefc60000,
    /// 18`) are MT19937 standard.
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
        self.draws += 1;
        y
    }

    /// A random value in the range `[0, max)`.
    ///
    /// Uses *Lemire's* multiply-shift trick instead of modulo: take the high 32
    /// bits of `random * max` (in 64 bits). This avoids modulo bias and is
    /// portable across stdlib implementations — exactly as rand.hpp does, which
    /// matters for determinism.
    pub fn bound(&mut self, max: u32) -> u32 {
        ((self.next_u32() as u64 * max as u64) >> 32) as u32
    }

    /// A random value in the range `[min, max)`.
    pub fn bound_range(&mut self, min: u32, max: u32) -> u32 {
        self.bound(max - min) + min
    }

    /// The most recently generated value (mirrors C++ `Rand::last`).
    /// Returns `0` immediately after `seed()`, or the value returned by the
    /// last `next_u32()` call.
    pub fn last(&self) -> u32 {
        self.last
    }

    /// Monotonic count of raw draws (`next_u32` calls) since construction.
    /// Diagnostic-only (no C++ counterpart, not hashed, not serialized): the
    /// differential harnesses take per-tick deltas to witness an RNG *burst*
    /// directly from the driven state.
    pub fn draws(&self) -> u64 {
        self.draws
    }
}

impl Default for Rand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_is_zero_after_seed() {
        let mut r = Rand::new();
        r.seed(42);
        assert_eq!(r.last(), 0, "last() must be 0 immediately after seed");
    }

    #[test]
    fn last_equals_most_recent_draw() {
        let mut r = Rand::new();
        r.seed(42);
        let v = r.next_u32();
        assert_eq!(r.last(), v, "last() must equal the value returned by next_u32()");
    }
}
