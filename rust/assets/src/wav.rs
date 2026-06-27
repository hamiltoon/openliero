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
}

#[cfg(test)]
mod smoke {
    use super::*;

    /// The real shipped bump.wav must decode, with original_data length equal to
    /// (file length - 44-byte header). Proves the fixed-header decode against a
    /// real file; failure here is a format-assumption signal, not a code bug.
    #[test]
    fn real_bump_wav_decodes() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero/sounds/bump.wav"
        ));
        let snd = WavSound::load(bytes).expect("bump.wav decodes");
        assert_eq!(snd.original_data.len(), bytes.len() - 44);
    }
}
