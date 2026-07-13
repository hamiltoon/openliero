//! `MultiBinary(7)` ↔ [`ControlState`] — the action mapper (design §2).
//!
//! The action space is the sim's native 7-bit `ControlState` word, exposed to
//! Python as `MultiBinary(7)`: seven independent Bernoulli bits, one per
//! `ControlState` control. [`decode`] builds a `ControlState` from the 7 bits;
//! [`encode`] is its exact inverse (used by the eval recorder, T5, and by the
//! round-trip tests below).
//!
//! **Bit ordering is EXACTLY `ControlState`'s named bit constants**
//! (`sim::state::ControlState`): index `n` of the `[bool; 7]` maps to control
//! bit `n`, i.e. `UP=0, DOWN=1, LEFT=2, RIGHT=3, FIRE=4, CHANGE=5, JUMP=6`. This
//! is a 1:1, lossless mapping — no discretization, no reduced alphabet (design
//! §2's `MultiBinary(7)` recommendation, chosen over `Discrete(128)` and the
//! engine's reduced 57-symbol alphabet).

use sim::state::ControlState;

/// Number of independent action bits (`MultiBinary(7)`, design §2).
pub const N_ACTION_BITS: u32 = 7;

/// Decode a `MultiBinary(7)` action into a [`ControlState`]. `bits[n]` sets
/// control bit `n` (`ControlState::UP..=ControlState::JUMP`, i.e. `0..=6`).
pub fn decode(bits: [bool; N_ACTION_BITS as usize]) -> ControlState {
    let mut cs = ControlState::new();
    for (n, &pressed) in bits.iter().enumerate() {
        cs.set(n as u32, pressed);
    }
    cs
}

/// Encode a [`ControlState`] back into its `MultiBinary(7)` bits — the exact
/// inverse of [`decode`]. Used by the eval recorder (T5) to tap the per-tick
/// action stream, and by the round-trip tests below.
pub fn encode(cs: ControlState) -> [bool; N_ACTION_BITS as usize] {
    let mut bits = [false; N_ACTION_BITS as usize];
    for (n, bit) in bits.iter_mut().enumerate() {
        *bit = cs.get(n as u32);
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Turn a 7-bit word `0..128` into its `[bool; 7]` bit array, LSB = bit 0
    /// (`ControlState::UP`) — the test-side inverse of `ControlState::pack()`,
    /// used to exhaustively drive `decode`/`encode` over every possible action.
    fn word_to_bits(word: u32) -> [bool; 7] {
        let mut bits = [false; 7];
        for (n, bit) in bits.iter_mut().enumerate() {
            *bit = (word >> n) & 1 != 0;
        }
        bits
    }

    /// RED (plan T2): each bit index must map to the named control it claims to
    /// — not just "some" bit. Pin every one individually.
    #[test]
    fn each_bit_maps_to_the_named_control() {
        let cases = [
            (ControlState::UP, "UP"),
            (ControlState::DOWN, "DOWN"),
            (ControlState::LEFT, "LEFT"),
            (ControlState::RIGHT, "RIGHT"),
            (ControlState::FIRE, "FIRE"),
            (ControlState::CHANGE, "CHANGE"),
            (ControlState::JUMP, "JUMP"),
        ];
        for (bit, name) in cases {
            let mut bits = [false; 7];
            bits[bit as usize] = true;
            let cs = decode(bits);
            for n in 0..7u32 {
                let expected = n == bit;
                assert_eq!(
                    cs.get(n),
                    expected,
                    "decoding only {name} (bit {bit}) set must leave every other bit \
                     clear (checked bit {n})"
                );
            }
        }
    }

    /// RED (plan T2): `decode` then `encode` must round-trip every one of the
    /// 128 possible 7-bit words — not just a handful of hand-picked cases.
    #[test]
    fn decode_encode_roundtrips_all_128_words() {
        for word in 0u32..128 {
            let bits = word_to_bits(word);
            let cs = decode(bits);
            assert_eq!(
                cs.pack(),
                word,
                "decode(bits for word {word:#09b}) must pack back to the same word"
            );
            assert_eq!(
                encode(cs),
                bits,
                "encode(decode(bits)) must be the identity for word {word:#09b}"
            );
        }
    }

    /// RED (plan T2): the other composition, `encode` then `decode`, must also
    /// be the identity for every packed `ControlState`.
    #[test]
    fn encode_decode_roundtrips_all_128_control_states() {
        for word in 0u32..128 {
            let cs = ControlState::unpack(word);
            let bits = encode(cs);
            assert_eq!(
                decode(bits).pack(),
                cs.pack(),
                "decode(encode(cs)) must reproduce cs for word {word:#09b}"
            );
        }
    }

    /// All-zero bits decode to the empty (no keys pressed) control state.
    #[test]
    fn all_false_decodes_to_empty_control_state() {
        assert_eq!(decode([false; 7]), ControlState::new());
    }
}
