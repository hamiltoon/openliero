//! Differential test for the tc.cfg loader against the C++ oracle. The golden
//! (one `label hash` line per group) is produced by the real C++ `Common::load`
//! (`LoadTcConfig`); the Rust loader must reproduce every FNV-1a digest from the
//! same shipped `data/TC/openliero/tc.cfg`. Digest byte layout matches the C++
//! dumper exactly (LE ints; strings = u32 LE length + bytes).

use std::collections::HashMap;

use assets::tc::{Bonus, ColorAnim, TcConfig, Texture};

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn push_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_i32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn push_str(b: &mut Vec<u8>, s: &str) {
    push_u32(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}

fn hash_names(names: &[String]) -> u64 {
    let mut b = Vec::new();
    push_u32(&mut b, names.len() as u32);
    for n in names {
        push_str(&mut b, n);
    }
    fnv1a(&b)
}

fn parse_golden(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split_whitespace();
            let label = it.next().unwrap().to_string();
            let hash = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (label, hash)
        })
        .collect()
}

#[test]
fn tc_config_matches_cpp_oracle() {
    let golden = parse_golden(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/tc.txt"
    )));
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/TC/openliero/tc.cfg"
    ));
    let cfg = TcConfig::load(bytes).expect("tc.cfg parses");

    let want = |k: &str| *golden.get(k).unwrap_or_else(|| panic!("missing golden {k}"));

    // types
    assert_eq!(hash_names(&cfg.types.sounds), want("types_sounds"));
    assert_eq!(hash_names(&cfg.types.weapons), want("types_weapons"));
    assert_eq!(hash_names(&cfg.types.nobjects), want("types_nobjects"));
    assert_eq!(hash_names(&cfg.types.sobjects), want("types_sobjects"));

    // constants (72 i32 in CDEFS order)
    {
        let mut b = Vec::new();
        for v in cfg.constants.ordered() {
            push_i32(&mut b, v);
        }
        assert_eq!(fnv1a(&b), want("constants"));
    }

    // materials (fixed [u8; 256] flags; mirrors the engine's materials[256])
    {
        assert_eq!(fnv1a(&cfg.materials), want("materials"));
    }

    // textures (9 × mframe,rframe,sframe i32 + ndrawback byte; pad to 9)
    {
        let mut t = cfg.textures.clone();
        t.resize(9, Texture::default());
        let mut b = Vec::new();
        for x in &t {
            push_i32(&mut b, x.mframe);
            push_i32(&mut b, x.rframe);
            push_i32(&mut b, x.sframe);
            b.push(if x.ndrawback { 1 } else { 0 });
        }
        assert_eq!(fnv1a(&b), want("textures"));
    }

    // bonuses (2 × timer,timerV,frame,sobj i32; pad to 2)
    {
        let mut bo = cfg.bonuses.clone();
        bo.resize(2, Bonus::default());
        let mut b = Vec::new();
        for x in &bo {
            push_i32(&mut b, x.timer);
            push_i32(&mut b, x.timer_v);
            push_i32(&mut b, x.frame);
            push_i32(&mut b, x.sobj);
        }
        assert_eq!(fnv1a(&b), want("bonuses"));
    }

    // coloranim (4 × from,to; pad to 4)
    {
        let mut ca = cfg.color_anim.clone();
        ca.resize(4, ColorAnim::default());
        let mut b = Vec::new();
        for x in &ca {
            push_i32(&mut b, x.from);
            push_i32(&mut b, x.to);
        }
        assert_eq!(fnv1a(&b), want("coloranim"));
    }

    // aiparams (7 × on,off; up..jump)
    {
        let mut b = Vec::new();
        for (on, off) in cfg.aiparams.ordered() {
            push_i32(&mut b, on);
            push_i32(&mut b, off);
        }
        assert_eq!(fnv1a(&b), want("aiparams"));
    }

    // texts (40 strings, SDEFS order)
    {
        let mut b = Vec::new();
        for s in cfg.texts.ordered() {
            push_str(&mut b, s);
        }
        assert_eq!(fnv1a(&b), want("texts"));
    }

    // hacks (11 bools, HDEFS order)
    {
        let mut b = Vec::new();
        for h in cfg.hacks.ordered() {
            b.push(if h { 1 } else { 0 });
        }
        assert_eq!(fnv1a(&b), want("hacks"));
    }

    // soundhooks (8 i32, SOUNDDEFS order)
    {
        let mut b = Vec::new();
        for i in cfg.sound_hooks.ordered() {
            push_i32(&mut b, i);
        }
        assert_eq!(fnv1a(&b), want("soundhooks"));
    }
}
