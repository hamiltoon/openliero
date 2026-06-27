//! WAV sound loading (`sounds/<name>.wav`). Reproduces the decoded 8-bit PCM
//! `original_data` that C++ `Common::load` (`src/game/common.cpp:325-363`)
//! stores for each sound, plus the `CreateSound` upsample
//! (`common.cpp:583-602`). Audio, NOT sim-affecting (no `processFrame` reads
//! sample data) — but "read the bytes the same": the decode is golden-pinned
//! vs C++. Idiomatic Rust (fixed-offset slicing + typed errors), not a port of
//! the streaming `io::Reader`.

/// Why a WAV failed to load.
#[derive(Debug, PartialEq, Eq)]
pub enum WavError {
    /// A header field did not match OpenLiero's single accepted WAV shape
    /// (`common.cpp:340-349`).
    BadHeader,
    /// The buffer ended before the 44-byte header or the declared PCM payload.
    Truncated,
}

/// A decoded OpenLiero sound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WavSound {
    /// 8-bit PCM, one byte per sample, each `= raw.wrapping_sub(128)`
    /// (equivalently `raw ^ 0x80`), exactly as C++ stores it
    /// (`common.cpp:354-356`). This is the LOCKED, golden-verified artifact.
    pub original_data: Vec<u8>,
}

/// Fixed canonical header length (`data` payload starts here).
const HEADER_LEN: usize = 44;

fn le_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn le_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

impl WavSound {
    /// Decode an OpenLiero `.wav`. Mirrors the load loop in `Common::load`
    /// (`common.cpp:340-356`): validate the fixed 44-byte RIFF/WAVE header
    /// (PCM, mono, 22050 Hz, 8-bit), then read `dataSize` bytes as `raw - 128`.
    pub fn load(bytes: &[u8]) -> Result<WavSound, WavError> {
        if bytes.len() < HEADER_LEN {
            return Err(WavError::Truncated);
        }
        // C++ reads 'RIFF', then the riff size (ignored), then validates the
        // remaining fields in one short-circuit `&&` chain. We check the same
        // fields at their fixed offsets.
        let ok = &bytes[0..4] == b"RIFF"
            // bytes[4..8] = riff size: read and ignored (common.cpp:341-343).
            && &bytes[8..12] == b"WAVE"
            && &bytes[12..16] == b"fmt "
            && le_u32(bytes, 16) == 16        // fmt chunk size
            && le_u16(bytes, 20) == 1         // audio format = PCM
            && le_u16(bytes, 22) == 1         // channels = mono
            && le_u32(bytes, 24) == 22050     // sample rate
            && le_u32(bytes, 28) == 22050     // byte rate (22050*1*1)
            && le_u16(bytes, 32) == 1         // block align (1*1)
            && le_u16(bytes, 34) == 8         // bits per sample
            && &bytes[36..40] == b"data";
        if !ok {
            return Err(WavError::BadHeader);
        }
        let data_size = le_u32(bytes, 40) as usize;
        let end = HEADER_LEN + data_size;
        if bytes.len() < end {
            return Err(WavError::Truncated);
        }
        // z = r.Get() - 128, wrapping in u8 (Get() returns uint8_t) == raw ^ 0x80.
        let original_data = bytes[HEADER_LEN..end]
            .iter()
            .map(|&b| b.wrapping_sub(128))
            .collect();
        Ok(WavSound { original_data })
    }

    /// The `int16` playback samples C++ `SfxSample::CreateSound`
    /// (`common.cpp:583-602`) produces: a 2x linear-interpolated upsample of
    /// `original_data * 30`. Each stored byte is reinterpreted as `int8`
    /// (undoing the `^ 0x80`), scaled by 30; an averaged tween is inserted
    /// between neighbours. Length is `2 * original_data.len()` (empty for an
    /// empty sound). Audio-only; SDL playback lives in step 3.
    pub fn upsampled(&self) -> Vec<i16> {
        if self.original_data.is_empty() {
            return Vec::new();
        }
        let mut samples = Vec::with_capacity(self.original_data.len() * 2);
        let mut prev = (self.original_data[0] as i8 as i32) * 30;
        samples.push(prev as i16);
        for &b in &self.original_data[1..] {
            let cur = (b as i8 as i32) * 30;
            samples.push(((prev + cur) / 2) as i16); // interpolated tween
            samples.push(cur as i16);
            prev = cur;
        }
        samples.push(prev as i16);
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid OpenLiero WAV (44-byte canonical header + `data`).
    fn wav(data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes()); // riff size
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&22050u32.to_le_bytes()); // sample rate
        v.extend_from_slice(&22050u32.to_le_bytes()); // byte rate
        v.extend_from_slice(&1u16.to_le_bytes()); // block align
        v.extend_from_slice(&8u16.to_le_bytes()); // bits
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes()); // data size
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn real_bump_wav_decodes() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sounds/bump.wav"
        ));
        let snd = WavSound::load(bytes).expect("bump.wav decodes");
        assert_eq!(snd.original_data.len(), bytes.len() - 44);
    }

    #[test]
    fn decode_offsets_each_byte_minus_128() {
        // raw 0x80->0x00, 0x00->0x80, 0xff->0x7f, 0x7c->0xfc (== raw ^ 0x80).
        let snd = WavSound::load(&wav(&[0x80, 0x00, 0xff, 0x7c])).unwrap();
        assert_eq!(snd.original_data, vec![0x00, 0x80, 0x7f, 0xfc]);
    }

    #[test]
    fn zero_length_data_is_ok_and_empty() {
        let snd = WavSound::load(&wav(&[])).unwrap();
        assert!(snd.original_data.is_empty());
        assert!(snd.upsampled().is_empty());
    }

    #[test]
    fn upsample_matches_create_sound() {
        // raw [0x80, 0x82] -> original_data [0, 2]; (int8)0*30=0, (int8)2*30=60.
        // CreateSound: push 0; tween (0+60)/2=30; push 60; trailing prev=60.
        let snd = WavSound::load(&wav(&[0x80, 0x82])).unwrap();
        assert_eq!(snd.original_data, vec![0u8, 2u8]);
        let up = snd.upsampled();
        assert_eq!(up, vec![0i16, 30, 60, 60]);
        assert_eq!(up.len(), snd.original_data.len() * 2);
    }

    #[test]
    fn upsample_handles_negative_int8() {
        // raw 0x7c -> original_data 0xfc -> (int8)0xfc = -4 -> -4*30 = -120.
        let snd = WavSound::load(&wav(&[0x7c])).unwrap();
        assert_eq!(snd.original_data, vec![0xfcu8]);
        assert_eq!(snd.upsampled(), vec![-120i16, -120]); // n=1 -> [prev, prev]
    }

    #[test]
    fn bad_header_fields_rejected() {
        // Each corruption of a validated field -> BadHeader.
        let mut bad_riff = wav(&[1, 2, 3]);
        bad_riff[0] = b'X';
        assert_eq!(WavSound::load(&bad_riff), Err(WavError::BadHeader));

        let mut bad_rate = wav(&[1, 2, 3]);
        bad_rate[24] = 0x44; // sample rate 0x4422 != 22050
        assert_eq!(WavSound::load(&bad_rate), Err(WavError::BadHeader));

        let mut bad_bits = wav(&[1, 2, 3]);
        bad_bits[34] = 16; // 16-bit, not 8
        assert_eq!(WavSound::load(&bad_bits), Err(WavError::BadHeader));

        let mut bad_data = wav(&[1, 2, 3]);
        bad_data[36] = b'L'; // "Lata" != "data"
        assert_eq!(WavSound::load(&bad_data), Err(WavError::BadHeader));
    }

    #[test]
    fn truncated_header_and_payload() {
        // Shorter than the 44-byte header.
        assert_eq!(WavSound::load(b"RIFF"), Err(WavError::Truncated));
        // Header claims 10 data bytes but only 3 are present.
        let mut short = wav(&[1, 2, 3]);
        let len = short.len();
        short[40..44].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(len, 47); // 44 + 3
        assert_eq!(WavSound::load(&short), Err(WavError::Truncated));
    }
}
