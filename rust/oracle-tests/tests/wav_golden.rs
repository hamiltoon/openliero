//! Differential test for the WAV loader against the C++ oracle. The golden (one
//! `name orig_len orig_hash up_len up_hash` line per sound) is produced by the
//! real C++ `Common::load`, which decodes original_data and calls CreateSound.
//! The Rust loader must reproduce every FNV-1a digest from the same shipped
//! `data/TC/openliero/sounds/<name>.wav`, walked in tc.cfg's sound order.

use std::collections::HashMap;

use assets::tc::TcConfig;
use assets::wav::WavSound;

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hash_i16(samples: &[i16]) -> u64 {
    let mut b = Vec::with_capacity(samples.len() * 2);
    for &v in samples {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fnv1a(&b)
}

struct Row {
    orig_len: usize,
    orig_hash: u64,
    up_len: usize,
    up_hash: u64,
}

fn parse_golden(text: &str) -> HashMap<String, Row> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next().unwrap().to_string();
            let orig_len = it.next().unwrap().parse().unwrap();
            let orig_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            let up_len = it.next().unwrap().parse().unwrap();
            let up_hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (name, Row { orig_len, orig_hash, up_len, up_hash })
        })
        .collect()
}

const TC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");

#[test]
fn wav_sounds_match_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/wav.txt"
    )));

    // Sound names + order come from tc.cfg (1e-1), exactly like C++ Common::load
    // (sounds[i].name = types.sounds[i]).
    let tc_bytes = std::fs::read(format!("{TC_DIR}/tc.cfg")).expect("read tc.cfg");
    let cfg = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    assert_eq!(cfg.types.sounds.len(), golden.len(), "sound count vs golden");

    for name in &cfg.types.sounds {
        let want = golden
            .get(name)
            .unwrap_or_else(|| panic!("missing golden line for {name}"));

        // Mirror Common::load's tolerance: a missing file is a silent slot.
        let path = format!("{TC_DIR}/sounds/{name}.wav");
        let snd = match std::fs::read(&path) {
            Ok(bytes) => WavSound::load(&bytes)
                .unwrap_or_else(|e| panic!("decode {name}: {e:?}")),
            Err(_) => WavSound::default(),
        };

        assert_eq!(snd.original_data.len(), want.orig_len, "{name} orig_len");
        assert_eq!(fnv1a(&snd.original_data), want.orig_hash, "{name} orig_hash");

        let up = snd.upsampled();
        assert_eq!(up.len(), want.up_len, "{name} up_len");
        assert_eq!(hash_i16(&up), want.up_hash, "{name} up_hash");
    }
}
