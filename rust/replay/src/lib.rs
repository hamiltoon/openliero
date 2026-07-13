//! Byte-faithful reader for the Liero `.lrp` replay format — **Phase 1**
//! (container + per-worm XOR-delta input stream + embedded
//! `WideRollbackChecksum` words).
//!
//! This mirrors the C++ `ReplayReader` read path (`src/game/replay.cpp`)
//! byte-for-byte for everything that does **not** require parsing cereal. Every
//! length-prefixed cereal blob — the initial `Game` graph and any mid-stream
//! `Settings`/`WormSettings` tag — is *skipped whole* via its `[uint32 len]`
//! prefix (`replay.cpp:60-71`), so the reader never has to understand cereal.
//! Parsing those (→ full-state reconstruction + `framehash` render diff) is
//! Phase 2, a deferred follow-on.
//!
//! ## What the stream carries (on the *inflated* bytes, `replay.cpp:99-326`)
//!
//! 1. `uint32 magic == 'LRPF'` — **big-endian** (`replay.cpp:112,116-119`).
//! 2. `uint8 version` — reject `> kMyReplayVersion` (`replay.cpp:120-123`).
//!    Phase 1 additionally rejects the pre-7 legacy branch (`< 7`); those files
//!    the C++ writer can no longer emit, and their palette/worm-rgb expansions
//!    are Phase-2 render concerns.
//! 3. initial `Game`: `[uint32 len][cereal blob]` — **skipped** (`replay.cpp:129`).
//! 4. per-frame stream (`replay.cpp:265-323`), repeated:
//!    - `0x80` empty frame — every worm keeps its last input (`replay.cpp:268`).
//!    - `0x81` settings — `[uint32 len][blob]`, **skipped** (`replay.cpp:271-279`).
//!    - `0x82` worm settings — `uint32 idx` + `[uint32 len][blob]`, **skipped**
//!      (`replay.cpp:280-289`).
//!    - `0x83` end — end of stream (`replay.cpp:290-292`).
//!    - `< 0x80` input frame — first byte is worm 0's delta, then **one byte per
//!      remaining worm** in `game.worms` order (`replay.cpp:293-307`). Per worm:
//!      `control_states = Unpack(byte ^ prev_control_states.Pack())`
//!      (`replay.cpp:304`), i.e. `(byte ^ prev) & 0x7f`.
//!    - anything else → error (`replay.cpp:309`).
//!    - **After** each input/empty frame, iff `cycles % (70*15) == 0` (every 1050
//!      frames) a `uint32 WideRollbackChecksum` word follows and is consumed
//!      (`replay.cpp:317-323`). The word is read at the *tail* of the playback
//!      step, against the state entering it — the same point the writer folds it
//!      (`replay.cpp:369-372`).
//!
//! ## Why `num_worms` is a parameter
//!
//! An input frame is one byte per worm with **no per-worm index** — the worm
//! count is implicit in the C++ loop over `game.worms`, which comes from the
//! (skipped) cereal `Game`. A Phase-1 reader therefore cannot infer it from the
//! stream; the caller supplies it (it reconstructs tick-0 from the same scenario
//! anyway, spec §5). All uint32s are **big-endian**, matching C++'s
//! `io::ReadUint32` (`io/coding.hpp:51-57`).
//!
//! ## The XOR baseline (load-bearing, `replay.cpp:304` + `game.cpp:466-468`)
//!
//! The delta XOR baseline is `prev_control_states`, which C++ updates at
//! `ProcessFrame`'s tail (`= control_states`). A pure stream reader that does not
//! run the sim maintains its own per-worm `prev = decoded control_states` after
//! each frame (spec §1). This is exact for a corpus whose inputs never provoke a
//! `ProcessFrame` control-state mutation (weapon-change / ninjarope throw) — the
//! committed corpus is built that way (fire straight down, no `Change`); any
//! drift would surface immediately as a decoded-input mismatch in the gate.

use std::collections::BTreeMap;

/// `('L'<<24)|('R'<<16)|('P'<<8)|'F'`, read big-endian (`replay.cpp:112`).
pub const REPLAY_MAGIC: u32 =
    (b'L' as u32) << 24 | (b'R' as u32) << 16 | (b'P' as u32) << 8 | b'F' as u32;

/// `kMyReplayVersion` (`version.hpp:7`). Versions above this are rejected
/// (`replay.cpp:121`).
pub const MAX_REPLAY_VERSION: u8 = 9;

/// The oldest version Phase 1 accepts. Pre-7 replays carried 6-bit VGA palette /
/// worm-rgb channels expanded on read (`replay.cpp:132-143`); that expansion is
/// palette-only (not folded by `WideRollbackChecksum`) and belongs to Phase 2,
/// so Phase 1 rejects `< 7` outright.
pub const MIN_REPLAY_VERSION: u8 = 7;

/// The checksum cadence: a `WideRollbackChecksum` word is embedded every
/// `70 * 15 == 1050` frames (`replay.cpp:317,369`).
pub const CHECKSUM_PERIOD: u64 = 70 * 15;

const TAG_EMPTY_FRAME: u8 = 0x80; // replay.cpp:23
const TAG_SETTINGS: u8 = 0x81; // replay.cpp:24
const TAG_WORM_SETTINGS: u8 = 0x82; // replay.cpp:25
const TAG_END: u8 = 0x83; // replay.cpp:26

/// Why a `.lrp` failed to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The zlib inflate failed (truncated / not a deflate stream). Carries the
    /// underlying message (kept as a `String` so the error stays `PartialEq`).
    Inflate(String),
    /// The leading magic was not big-endian `'LRPF'` (`replay.cpp:117`).
    BadMagic { found: u32 },
    /// `version > kMyReplayVersion` (`replay.cpp:121`).
    VersionTooRecent { version: u8 },
    /// `version < 7` — the pre-7 legacy branch, deferred to Phase 2.
    VersionTooOld { version: u8 },
    /// The stream ended before a field/frame was complete.
    UnexpectedEof,
    /// A frame header byte in `0x84..=0xFF` (no such tag, `replay.cpp:309`).
    UnexpectedHeaderByte(u8),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Inflate(m) => write!(f, "replay inflate failed: {m}"),
            ReplayError::BadMagic { found } => {
                write!(f, "not a replay: bad magic {found:#010x}")
            }
            ReplayError::VersionTooRecent { version } => {
                write!(
                    f,
                    "replay version {version} is too recent (max {MAX_REPLAY_VERSION})"
                )
            }
            ReplayError::VersionTooOld { version } => {
                write!(
                    f,
                    "replay version {version} is a pre-7 legacy file (Phase 2)"
                )
            }
            ReplayError::UnexpectedEof => write!(f, "replay stream ended unexpectedly"),
            ReplayError::UnexpectedHeaderByte(b) => {
                write!(f, "unexpected replay header byte {b:#04x}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// One decoded playback frame (one simulation tick).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The decoded 7-bit `control_states.istate` per worm, in `game.worms`
    /// order — the value the C++ writer set before `RecordFrame`.
    pub inputs: Vec<u32>,
    /// The per-worm XOR baseline (`prev_control_states.istate`) *entering* this
    /// frame — i.e. the previous frame's decoded inputs (all `0` at tick 0).
    /// This is the value `WideRollbackChecksum` folds for `prev_control_states`
    /// at a checksum boundary (`replay.cpp:206`); the Phase-1 gate passes it to
    /// `sim::wide_checksum::wide_rollback_checksum` as `prev_istates` (the sim
    /// crate is deliberately not a dependency of this crate).
    pub prev_inputs: Vec<u32>,
}

/// A decoded `.lrp` replay (Phase 1): the per-tick input stream plus the
/// embedded checksum words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// The `.lrp` format version byte (`replay.cpp:120`).
    pub version: u8,
    /// The worm count the stream was decoded with (caller-supplied).
    pub num_worms: usize,
    /// One entry per tick, in stream order.
    pub frames: Vec<Frame>,
    /// `cycle -> embedded WideRollbackChecksum word`, one per `cycle % 1050 == 0`
    /// boundary reached before the `0x83` end (`replay.cpp:317-323`).
    pub checksums: BTreeMap<u64, u32>,
}

impl Replay {
    /// Number of decoded ticks (frames before the `0x83` end).
    pub fn ticks(&self) -> usize {
        self.frames.len()
    }

    /// Inflate a zlib-wrapped `.lrp` file and parse it. `num_worms` must be
    /// supplied externally (see the module docs).
    pub fn parse(compressed: &[u8], num_worms: usize) -> Result<Replay, ReplayError> {
        let data = inflate(compressed)?;
        Replay::parse_inflated(&data, num_worms)
    }

    /// Parse an already-inflated `.lrp` byte stream. Split out from [`parse`] so
    /// hand-crafted byte fixtures can exercise the framing without deflate.
    pub fn parse_inflated(data: &[u8], num_worms: usize) -> Result<Replay, ReplayError> {
        let mut c = Cursor::new(data);

        // Container header (replay.cpp:112-123).
        let magic = c.u32_be()?;
        if magic != REPLAY_MAGIC {
            return Err(ReplayError::BadMagic { found: magic });
        }
        let version = c.u8()?;
        if version > MAX_REPLAY_VERSION {
            return Err(ReplayError::VersionTooRecent { version });
        }
        if version < MIN_REPLAY_VERSION {
            return Err(ReplayError::VersionTooOld { version });
        }

        // Skip the initial `Game` cereal blob via its length prefix
        // (replay.cpp:129 -> CerealRead, replay.cpp:76-77).
        let game_len = c.u32_be()? as usize;
        c.skip(game_len)?;

        // Per-worm XOR baseline (`prev_control_states.istate`), tick-0 value 0.
        let mut prev = vec![0u32; num_worms];
        let mut frames: Vec<Frame> = Vec::new();
        let mut checksums: BTreeMap<u64, u32> = BTreeMap::new();
        let mut cycle: u64 = 0;

        // Playback loop (replay.cpp:265-323): read tagged records / input frames
        // until the 0x83 end. Settings tags (0x81/0x82) are skipped in place and
        // do NOT count as a frame (they `continue` the inner C++ while-loop);
        // only an input or empty frame advances the cycle + consumes a checksum.
        'stream: loop {
            let inputs;
            let prev_inputs;

            // Inner loop mirrors the C++ `while (true)` (replay.cpp:265): keep
            // reading records until one produces a frame (or the stream ends).
            loop {
                let first = c.u8()?;
                if first == TAG_EMPTY_FRAME {
                    // replay.cpp:268 — no input change; every worm keeps its last
                    // input (control_states unchanged, so prev is unchanged too).
                    prev_inputs = prev.clone();
                    inputs = prev.clone();
                    break;
                } else if first == TAG_SETTINGS {
                    // replay.cpp:271-279 — [u32 len][cereal Settings]; skipped.
                    let len = c.u32_be()? as usize;
                    c.skip(len)?;
                    continue;
                } else if first == TAG_WORM_SETTINGS {
                    // replay.cpp:280-289 — u32 worm_idx + [u32 len][WormSettings];
                    // both skipped.
                    let _worm_idx = c.u32_be()?;
                    let len = c.u32_be()? as usize;
                    c.skip(len)?;
                    continue;
                } else if first == TAG_END {
                    // replay.cpp:290-292 — end of stream.
                    break 'stream;
                } else if first < TAG_EMPTY_FRAME {
                    // replay.cpp:293-307 — input frame: `first` is worm 0's delta,
                    // then one byte per remaining worm in game.worms order. Per
                    // worm: control_states = Unpack(byte ^ prev.Pack()), i.e.
                    // (byte ^ prev) & 0x7f (Unpack masks 7 bits, worm.hpp:159).
                    prev_inputs = prev.clone();
                    let mut decoded = Vec::with_capacity(num_worms);
                    for (i, prev_i) in prev.iter_mut().enumerate() {
                        let byte = if i == 0 { first } else { c.u8()? };
                        let istate = (byte as u32 ^ *prev_i) & 0x7f;
                        decoded.push(istate);
                        *prev_i = istate; // baseline for the next frame
                    }
                    inputs = decoded;
                    break;
                } else {
                    // replay.cpp:309 — 0x84..=0xFF is not a valid header byte.
                    return Err(ReplayError::UnexpectedHeaderByte(first));
                }
            }

            frames.push(Frame {
                inputs,
                prev_inputs,
            });

            // replay.cpp:317-323 — every 1050 frames a checksum word follows the
            // frame, folded against the state entering this playback step
            // (C++ `game.cycles % (70*15) == 0`).
            if cycle.is_multiple_of(CHECKSUM_PERIOD) {
                let word = c.u32_be()?;
                checksums.insert(cycle, word);
            }
            cycle += 1;
        }

        Ok(Replay {
            version,
            num_worms,
            frames,
            checksums,
        })
    }
}

/// Inflate the whole zlib-wrapped deflate stream to memory, mirroring C++'s
/// up-front `InflateReader` drain (`replay.cpp:99-110`). miniz's default
/// `mz_deflateInit` emits a zlib wrapper (`0x78 …` + adler32), so a `ZlibDecoder`
/// is the correct decoder (`deflate.hpp:3-4,136`; spec §4).
pub fn inflate(compressed: &[u8]) -> Result<Vec<u8>, ReplayError> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(compressed)
        .read_to_end(&mut out)
        .map_err(|e| ReplayError::Inflate(e.to_string()))?;
    Ok(out)
}

/// A forward-only big-endian byte cursor over the inflated stream (mirrors the
/// C++ `io::MemReader` the reader drives, `replay.cpp:109`).
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, ReplayError> {
        let b = *self.data.get(self.pos).ok_or(ReplayError::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }

    /// Big-endian `uint32`, matching C++ `io::ReadUint32` (`io/coding.hpp:51`).
    fn u32_be(&mut self) -> Result<u32, ReplayError> {
        let end = self.pos + 4;
        let bytes = self
            .data
            .get(self.pos..end)
            .ok_or(ReplayError::UnexpectedEof)?;
        self.pos = end;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn skip(&mut self, n: usize) -> Result<(), ReplayError> {
        let end = self.pos.checked_add(n).ok_or(ReplayError::UnexpectedEof)?;
        if end > self.data.len() {
            return Err(ReplayError::UnexpectedEof);
        }
        self.pos = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- hand-crafted byte-fixture helpers ---------------------------------

    /// Big-endian u32, matching the on-stream encoding.
    fn be(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    /// Build an inflated `.lrp` byte stream: magic + version + an empty
    /// (`len == 0`) initial `Game` blob + `body` + a trailing `0x83`.
    fn stream(version: u8, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&be(REPLAY_MAGIC));
        v.push(version);
        v.extend_from_slice(&be(0)); // empty cereal Game blob
        v.extend_from_slice(body);
        v.push(TAG_END);
        v
    }

    #[test]
    fn bad_magic_rejected() {
        let mut data = stream(9, &[]);
        data[0] ^= 0xFF; // corrupt the magic
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::BadMagic {
                found: u32::from_be_bytes([data[0], data[1], data[2], data[3]])
            })
        );
    }

    #[test]
    fn version_too_recent_rejected() {
        let data = stream(MAX_REPLAY_VERSION + 1, &[]);
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::VersionTooRecent {
                version: MAX_REPLAY_VERSION + 1
            })
        );
    }

    #[test]
    fn version_too_old_rejected() {
        // Pre-7 legacy is a Phase-2 concern; Phase 1 rejects it.
        let data = stream(6, &[]);
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::VersionTooOld { version: 6 })
        );
        // The boundary (7) is accepted.
        assert!(Replay::parse_inflated(&stream(7, &[]), 2).is_ok());
        assert!(Replay::parse_inflated(&stream(MAX_REPLAY_VERSION, &[]), 2).is_ok());
    }

    #[test]
    fn truncated_stream_errors() {
        // Magic + version only, then nothing (missing the Game length prefix).
        let mut data = Vec::new();
        data.extend_from_slice(&be(REPLAY_MAGIC));
        data.push(9);
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    // Every stream folds a checksum word after frame 0 (cycle 0 % 1050 == 0), so
    // multi-frame fixtures must carry it — exactly as the C++ writer does.
    const CYCLE0: u32 = 0xDEAD_BEEF;

    #[test]
    fn xor_delta_decode_two_worms_and_end() {
        // Two worms. Frame 0: bytes [2, 2] with prev 0 -> inputs [2, 2]; then the
        // cycle-0 checksum word.
        // Frame 1: bytes [16, 16] with prev [2, 2] -> inputs [16^2, 16^2] = [18, 18].
        // Frame 2: bytes [18, 18] with prev [18, 18] -> inputs [0, 0].
        let mut body = Vec::new();
        body.extend_from_slice(&[2, 2]);
        body.extend_from_slice(&be(CYCLE0));
        body.extend_from_slice(&[16, 16, 18, 18]);
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.version, 9);
        assert_eq!(replay.num_worms, 2);
        assert_eq!(replay.ticks(), 3);
        assert_eq!(replay.frames[0].inputs, vec![2, 2]);
        assert_eq!(replay.frames[0].prev_inputs, vec![0, 0]);
        assert_eq!(replay.frames[1].inputs, vec![18, 18]);
        assert_eq!(replay.frames[1].prev_inputs, vec![2, 2]);
        assert_eq!(replay.frames[2].inputs, vec![0, 0]);
        assert_eq!(replay.frames[2].prev_inputs, vec![18, 18]);
        assert_eq!(replay.checksums.get(&0), Some(&CYCLE0));
    }

    #[test]
    fn empty_frame_repeats_last_input() {
        // Frame 0: [3, 5] + cycle-0 checksum. Frame 1: 0x80 empty -> inputs
        // unchanged [3, 5]. Frame 2: [3, 5] with prev [3, 5] -> [0, 0].
        let mut body = Vec::new();
        body.extend_from_slice(&[3, 5]);
        body.extend_from_slice(&be(CYCLE0));
        body.push(TAG_EMPTY_FRAME);
        body.extend_from_slice(&[3, 5]);
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.ticks(), 3);
        assert_eq!(replay.frames[0].inputs, vec![3, 5]);
        assert_eq!(
            replay.frames[1].inputs,
            vec![3, 5],
            "empty frame repeats last input"
        );
        assert_eq!(replay.frames[1].prev_inputs, vec![3, 5]);
        assert_eq!(replay.frames[2].inputs, vec![0, 0]);
    }

    #[test]
    fn settings_tag_skipped_without_disturbing_next_frame() {
        // 0x81 [len=3][blob 3B] then a normal frame + its cycle-0 checksum. The
        // settings skip must land the cursor exactly on the frame byte, and the
        // tag itself must not advance the cycle (else the checksum misaligns).
        let mut body = Vec::new();
        body.push(TAG_SETTINGS);
        body.extend_from_slice(&be(3));
        body.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        body.extend_from_slice(&[7, 9]); // frame: prev 0 -> [7, 9]
        body.extend_from_slice(&be(CYCLE0));
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.ticks(), 1, "the settings tag is not a frame");
        assert_eq!(replay.frames[0].inputs, vec![7, 9]);
        assert_eq!(replay.checksums.get(&0), Some(&CYCLE0));
    }

    #[test]
    fn worm_settings_tag_skips_idx_and_blob() {
        // 0x82 [u32 worm_idx][u32 len][blob] then a frame + its cycle-0 checksum.
        let mut body = Vec::new();
        body.push(TAG_WORM_SETTINGS);
        body.extend_from_slice(&be(1)); // worm_idx
        body.extend_from_slice(&be(2)); // blob len
        body.extend_from_slice(&[0xDE, 0xAD]);
        body.extend_from_slice(&[4, 8]);
        body.extend_from_slice(&be(CYCLE0));
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.ticks(), 1);
        assert_eq!(replay.frames[0].inputs, vec![4, 8]);
    }

    #[test]
    fn unexpected_header_byte_errors() {
        // 0x84 is neither a delta (< 0x80) nor a known tag (0x80..=0x83).
        let body = [0x84u8];
        let mut data = Vec::new();
        data.extend_from_slice(&be(REPLAY_MAGIC));
        data.push(9);
        data.extend_from_slice(&be(0));
        data.extend_from_slice(&body);
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedHeaderByte(0x84))
        );
    }

    #[test]
    fn checksum_word_captured_at_cycle_zero() {
        // Frame 0 (cycle 0) is a checksum boundary: a big-endian word follows it.
        let mut body = Vec::new();
        body.extend_from_slice(&[2, 2]); // frame 0
        body.extend_from_slice(&be(0x0310_57d7)); // cycle-0 checksum word
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.ticks(), 1);
        assert_eq!(replay.checksums.get(&0), Some(&0x0310_57d7));
    }

    #[test]
    fn checksum_boundary_lands_at_cycle_1050() {
        // Programmatically build 1051 frames (2 worms, all zero-delta) with a
        // checksum word after frame 0 and after frame 1050 — pinning the 1050
        // cadence without the corpus.
        let mut body = Vec::new();
        for cycle in 0..=CHECKSUM_PERIOD {
            body.extend_from_slice(&[0, 0]); // zero-delta frame
            if cycle.is_multiple_of(CHECKSUM_PERIOD) {
                body.extend_from_slice(&be(0xC0DE_0000 + cycle as u32));
            }
        }
        let replay = Replay::parse_inflated(&stream(9, &body), 2).unwrap();
        assert_eq!(replay.ticks(), (CHECKSUM_PERIOD + 1) as usize);
        assert_eq!(replay.checksums.len(), 2);
        assert_eq!(replay.checksums.get(&0), Some(&0xC0DE_0000));
        assert_eq!(
            replay.checksums.get(&CHECKSUM_PERIOD),
            Some(&(0xC0DE_0000 + CHECKSUM_PERIOD as u32))
        );
    }

    // --- hostile-input hardening (T4) --------------------------------------
    //
    // The reader will take arbitrary files from a future CLI, so every
    // malformed stream must yield a clean `Err` — never a panic, an over-read,
    // or an allocation blow-up. The forward-only checked `Cursor` (`u8`/`u32_be`
    // bounds-check via `get`; `skip` uses `checked_add` + a `len()` bound)
    // already guarantees this structurally; these tests pin each hostile path,
    // covering the gaps the generated corpus and T2's happy-path fixtures leave.
    // Every field/frame read short-circuits to `UnexpectedEof`, mirroring the
    // C++ `MemReader` throwing `EndOfStream` past its buffer (stream.hpp:177).

    /// A header (magic + version) with no initial-`Game` length prefix at all.
    fn header_only(version: u8) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&be(REPLAY_MAGIC));
        v.push(version);
        v.extend_from_slice(&be(0)); // empty Game blob
        v
    }

    #[test]
    fn truncated_mid_checksum_word_errors() {
        // Frame 0 (cycle 0) demands a trailing 4-byte checksum word; supply only
        // two of its bytes -> the `u32_be` read must fail, not over-read.
        let mut data = header_only(9);
        data.extend_from_slice(&[2, 2]); // frame 0
        data.extend_from_slice(&[0xDE, 0xAD]); // only 2 of 4 checksum bytes
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn truncated_cereal_length_prefix_errors() {
        // Magic + version, then only two bytes of the initial `Game`'s uint32
        // length prefix -> the length read itself must fail cleanly.
        let mut data = Vec::new();
        data.extend_from_slice(&be(REPLAY_MAGIC));
        data.push(9);
        data.extend_from_slice(&[0x00, 0x10]); // partial u32 length prefix
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn giant_cereal_length_prefix_errors_without_oom() {
        // A hostile `Game` length of 0xFFFF_FFFF (~4 GiB) must NOT allocate or
        // over-read: `skip` bounds-checks against the remaining bytes (its
        // `checked_add` also guards the usize overflow on 32-bit wasm), so this
        // resolves to a plain EOF error in O(1) with no allocation.
        let mut data = Vec::new();
        data.extend_from_slice(&be(REPLAY_MAGIC));
        data.push(9);
        data.extend_from_slice(&be(u32::MAX)); // absurd Game blob length
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn giant_settings_length_prefix_errors_without_oom() {
        // The same guard on a mid-stream `0x81` settings blob length.
        let mut body = Vec::new();
        body.push(TAG_SETTINGS);
        body.extend_from_slice(&be(u32::MAX));
        // `stream()` appends 0x83, but the skip fails long before we reach it.
        assert_eq!(
            Replay::parse_inflated(&stream(9, &body), 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn truncated_input_frame_errors() {
        // Two worms, but the frame carries only worm 0's delta byte; worm 1's
        // byte is missing -> the per-worm read must fail, not read past the end.
        let mut data = header_only(9);
        data.push(2); // worm 0 delta only; worm 1 byte absent
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn truncated_worm_settings_header_errors() {
        // `0x82` promises [u32 worm_idx][u32 len][blob]; give only two bytes of
        // the worm index.
        let mut data = header_only(9);
        data.push(TAG_WORM_SETTINGS);
        data.extend_from_slice(&[0x00, 0x01]); // partial worm_idx u32
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn unterminated_stream_errors() {
        // A frame + its cycle-0 checksum but no `0x83` end tag: the next inner
        // `u8` read hits EOF and must error (mirroring the C++ `MemReader`
        // throwing past its buffer) rather than loop forever.
        let mut data = header_only(9);
        data.extend_from_slice(&[1, 1]); // frame 0
        data.extend_from_slice(&be(CYCLE0)); // cycle-0 checksum
                                             // (no TAG_END)
        assert_eq!(
            Replay::parse_inflated(&data, 2),
            Err(ReplayError::UnexpectedEof)
        );
    }

    #[test]
    fn trailing_bytes_after_end_are_ignored() {
        // C++ stops reading at `0x83` (PlaybackFrame returns false,
        // replay.cpp:290); anything after the end tag — including a second
        // `0x83` — is never read.
        let mut data = header_only(9);
        data.extend_from_slice(&[5, 6]); // frame 0
        data.extend_from_slice(&be(CYCLE0)); // cycle-0 checksum
        data.push(TAG_END); // end of stream
        data.extend_from_slice(&[TAG_END, 0xFF, 0x00]); // ignored trailing bytes
        let replay = Replay::parse_inflated(&data, 2).unwrap();
        assert_eq!(replay.ticks(), 1);
        assert_eq!(replay.frames[0].inputs, vec![5, 6]);
    }

    #[test]
    fn zero_worms_terminates_cleanly() {
        // A degenerate caller worm count: the reader must still terminate on the
        // end tag with zero frames rather than panic or underflow.
        let replay = Replay::parse_inflated(&stream(9, &[]), 0).unwrap();
        assert_eq!(replay.ticks(), 0);
        assert_eq!(replay.num_worms, 0);
    }

    #[test]
    fn non_zlib_data_is_inflate_error_not_panic() {
        // The compressed entry point on hostile bytes: a non-deflate buffer must
        // surface as `ReplayError::Inflate`, never a panic.
        let junk = [0u8, 1, 2, 3, 4, 5, 6, 7];
        match Replay::parse(&junk, 2) {
            Err(ReplayError::Inflate(_)) => {}
            other => panic!("expected Inflate error, got {other:?}"),
        }
    }
}
