//! Differential test for random level generation (Step 4½, slice 4½b) against the C++
//! oracle `oracle_dump_levelgen` (`src/tools/oracle_dump/levelgen_dump.cpp`, regenerated
//! LOCALLY by `gen_levelgen_golden.sh`). The golden is the single source of truth for the
//! matrix. Every `gen` line is replayed stage by stage, so a failure names the FIRST
//! diverging stage; every stage token pins the level hash AND `rand.last`.
//!
//!   gen <seed> <w> <h> <shadow> <field> <splats> <stones> <tunnels> <formations> <rocks>
//!       <shadowed> <dig> <form_count> <form_placed> <form_tries> <rock_count>
//!       <rock_placed> <rock_tries>
//!   file <level_file> <seed> <shadow> <w> <h> <final>
//!
//! stage token = `<fnv1a64(material_id) %016x>:<rand.last %08x>`; `<shadowed>` = bare hash
//! after MakeShadow or `-`; `<dig>` = 12 DrawDirtEffect(7) stamps (+ CorrectShadow when
//! shadow) continuing the generation RNG (worm.cpp:931-934 shape). The `shadow=1` dig tokens
//! need 4½a's `sim::shadow::correct_shadow` and are checked by the second test (T9).
//! `<level_file>` is relative to the repo root; two `file` lines load the dumper-written
//! fixture `golden/levelgen_shadow_fixture.lev` (the pre-MakeShadow `gen 42 128 96` map) with
//! shadow 1 and 0, so the file branch's MakeShadow wiring is a check that can fail.

use std::collections::{BTreeMap, BTreeSet};

use assets::sprite::{SpriteSet, Tga};
use assets::tc::{TcConfig, Texture};
use sim::blit::draw_dirt_effect;
use sim::levelgen::{self, make_shadow, LevelGenAssets, LevelGenParams, RockStats};
use sim::state::LevelSim;
use sim_core::rng::Rand;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The dumper's MakeShadow fixture (`levelgen_dump.cpp` `kShadowFixture`), repo-root-relative.
const SHADOW_FIXTURE: &str = "rust/oracle-tests/golden/levelgen_shadow_fixture.lev";
/// The `gen` case the fixture serialises (`levelgen_dump.cpp` `DumpShadowFixture`).
const FIXTURE_CASE: (u32, i32, i32) = (42, 128, 96);

/// The level golden's FNV-1a 64 (`level_dump.cpp:21-28`).
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn stage(material_id: &[u8], rand: &Rand) -> String {
    format!("{:016x}:{:08x}", fnv1a(material_id), rand.last())
}

fn seeded(seed: u32) -> Rand {
    let mut r = Rand::new();
    r.seed(seed);
    r
}

struct Tc {
    large: SpriteSet,
    textures: Vec<Texture>,
    flags: [u8; 256],
}

/// The same TC the dumper's `Common::load("data/TC/openliero")` reads.
fn load_tc() -> Tc {
    let tc_root = format!("{ROOT}/data/TC/openliero");
    let tc_bytes = std::fs::read(format!("{tc_root}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let tga_bytes = std::fs::read(format!("{tc_root}/sprites/large.tga")).expect("read large.tga");
    let tga = Tga::load(&tga_bytes).expect("large.tga parses");
    let large = SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank");
    Tc {
        large,
        textures: tc.textures.clone(),
        flags: tc.materials,
    }
}

/// Mirror of the dumper's `Dig`: 12 dig-texture stamps at `((i*41+5) % w - 7,
/// (i*29+7) % h - 7)`, each followed by `after_stamp(level, x, y)` — nothing for
/// `shadow=0`, CorrectShadow over `Rect(x-3, y-3, x+18, y+18)` for `shadow=1` (T9).
fn dig_stamps(
    level: &mut LevelSim,
    tc: &Tc,
    rand: &mut Rand,
    mut after_stamp: impl FnMut(&mut LevelSim, i32, i32),
) {
    for i in 0..12i32 {
        let x = (i * 41 + 5) % level.width - 7;
        let y = (i * 29 + 7) % level.height - 7;
        draw_dirt_effect(level, &tc.large, &tc.textures, 7, x, y, rand);
        after_stamp(&mut *level, x, y);
    }
}

fn rock_stats(cols: &[&str]) -> RockStats {
    RockStats {
        count: cols[0].parse().expect("count"),
        placed: cols[1].parse().expect("placed"),
        tries: cols[2].parse().expect("tries"),
    }
}

#[derive(Default)]
struct Coverage {
    combos: BTreeSet<(i32, i32, bool)>,
    gen_lines: usize,
    file_lines: usize,
    capped: usize,
    retried: usize,
    file_loaded: usize,
    file_fallback: usize,
    /// `FIXTURE_CASE`'s golden (pre-shadow `<rocks>` hash, `<shadowed>` hash), from its
    /// shadow=1 `gen` line.
    fixture_source: Option<(String, String)>,
    /// shadow -> final hash of the `SHADOW_FIXTURE` file lines, as the Rust port computed it.
    fixture_final: BTreeMap<bool, String>,
}

fn check_gen(n: usize, t: &[&str], tc: &Tc, cov: &mut Coverage) {
    assert_eq!(t.len(), 19, "line {n}: a gen record has 19 fields");
    let seed: u32 = t[1].parse().expect("seed");
    let w: i32 = t[2].parse().expect("w");
    let h: i32 = t[3].parse().expect("h");
    let shadow = match t[4] {
        "0" => false,
        "1" => true,
        other => panic!("line {n}: shadow must be 0/1, got {other}"),
    };
    let ctx = format!("line {n} (gen seed {seed} {w}x{h} shadow {})", t[4]);
    let assets = LevelGenAssets {
        large_sprites: &tc.large,
        textures: &tc.textures,
        material_flags: &tc.flags,
    };

    // The six stages, each checked the moment it finishes.
    let mut rand = seeded(seed);
    let mut level = levelgen::new_level(w, h, &tc.flags);
    levelgen::generate_dirt_field(&mut level, &mut rand);
    assert_eq!(
        stage(&level.material_id, &rand),
        t[5],
        "{ctx}: stage `field` (level.cpp:12-26)"
    );
    levelgen::splat_large_sprites(&mut level, &tc.large, &mut rand);
    assert_eq!(
        stage(&level.material_id, &rand),
        t[6],
        "{ctx}: stage `splats` (level.cpp:30-71)"
    );
    levelgen::scatter_stones(&mut level, &tc.large, &mut rand);
    assert_eq!(
        stage(&level.material_id, &rand),
        t[7],
        "{ctx}: stage `stones` (level.cpp:73-82)"
    );
    levelgen::dig_tunnels(&mut level, &tc.large, &tc.textures, &mut rand);
    assert_eq!(
        stage(&level.material_id, &rand),
        t[8],
        "{ctx}: stage `tunnels` (level.cpp:108-135)"
    );
    let form = levelgen::place_rock_formations(&mut level, &tc.large, &mut rand);
    assert_eq!(
        form,
        rock_stats(&t[13..16]),
        "{ctx}: formation count/placed/tries"
    );
    assert_eq!(
        stage(&level.material_id, &rand),
        t[9],
        "{ctx}: stage `formations` (level.cpp:137-170)"
    );
    let rocks = levelgen::place_rocks(&mut level, &tc.large, &mut rand);
    assert_eq!(
        rocks,
        rock_stats(&t[16..19]),
        "{ctx}: rock count/placed/tries"
    );
    assert_eq!(
        stage(&level.material_id, &rand),
        t[10],
        "{ctx}: stage `rocks` (level.cpp:172-192)"
    );

    // The composed entry point equals the staged run.
    let mut r2 = seeded(seed);
    let composed = levelgen::generate_random(&assets, w, h, &mut r2);
    assert_eq!(
        stage(&composed.material_id, &r2),
        t[10],
        "{ctx}: generate_random != staged run"
    );

    // MakeShadow (no RNG).
    if shadow {
        make_shadow(&mut level);
        let shadowed = format!("{:016x}", fnv1a(&level.material_id));
        assert_eq!(shadowed, t[11], "{ctx}: MakeShadow (level.cpp:195-216)");
        assert_ne!(
            t[11],
            &t[10][..16],
            "{ctx}: MakeShadow changed nothing (vacuous)"
        );
        if (seed, w, h) == FIXTURE_CASE {
            cov.fixture_source = Some((t[10][..16].to_string(), t[11].to_string()));
        }
    } else {
        assert_eq!(t[11], "-", "{ctx}: shadow=0 carries no shadowed hash");
    }

    // The shell's entry point: GenerateFromSettings(random) == generation (+ MakeShadow).
    let params = LevelGenParams {
        random_level: true,
        random_map_width: w,
        random_map_height: h,
        shadow,
    };
    let mut r3 = seeded(seed);
    let data = levelgen::generate_from_settings(&assets, &params, None, &mut r3);
    assert_eq!(
        data.material_id, level.material_id,
        "{ctx}: generate_from_settings != staged (+ shadow)"
    );
    assert_eq!(
        r3.last(),
        rand.last(),
        "{ctx}: generate_from_settings RNG position"
    );
    assert!(
        data.palette.is_none() && data.display.is_none(),
        "{ctx}: generated level carries no palette/display"
    );

    // Dig stage, continuing the generation RNG. shadow=0: DrawDirtEffect 7 only. shadow=1 adds
    // CorrectShadow (4½a's port) and is checked by `levelgen_dig_stage_with_correct_shadow` (T9).
    if !shadow {
        dig_stamps(&mut level, tc, &mut rand, |_, _, _| {});
        assert_eq!(
            stage(&level.material_id, &rand),
            t[12],
            "{ctx}: dig stage (blit.cpp:534-622)"
        );
    }

    let capped = form.placed < form.count || rocks.placed < rocks.count;
    if capped {
        cov.capped += 1;
    } else if form.tries > form.count as u64 || rocks.tries > rocks.count as u64 {
        cov.retried += 1;
    }
    cov.combos.insert((w, h, shadow));
    cov.gen_lines += 1;
}

fn check_file(n: usize, t: &[&str], tc: &Tc, cov: &mut Coverage) {
    assert_eq!(t.len(), 7, "line {n}: a file record has 7 fields");
    let level_file = t[1];
    let seed: u32 = t[2].parse().expect("seed");
    let shadow = match t[3] {
        "0" => false,
        "1" => true,
        other => panic!("line {n}: shadow must be 0/1, got {other}"),
    };
    let w: i32 = t[4].parse().expect("w");
    let h: i32 = t[5].parse().expect("h");
    let ctx = format!("line {n} (file {level_file} seed {seed} shadow {})", t[3]);
    let assets = LevelGenAssets {
        large_sprites: &tc.large,
        textures: &tc.textures,
        material_flags: &tc.flags,
    };

    // The caller-side I/O of GenerateFromSettings: name rule, read, parse; any failure => None.
    let name = levelgen::level_file_name(level_file);
    let file = std::fs::read(format!("{ROOT}/{name}"))
        .ok()
        .and_then(|b| assets::level::load(&b).ok());
    let loaded = file.is_some();
    let params = LevelGenParams {
        random_level: false,
        shadow,
        ..LevelGenParams::default()
    };
    let mut rand = seeded(seed);
    let data = levelgen::generate_from_settings(&assets, &params, file, &mut rand);
    assert_eq!((data.width, data.height), (w, h), "{ctx}: size");
    assert_eq!(
        stage(&data.material_id, &rand),
        t[6],
        "{ctx}: GenerateFromSettings (level.cpp:397-429)"
    );
    if loaded {
        assert_eq!(rand.last(), 0, "{ctx}: the file branch draws no RNG");
        cov.file_loaded += 1;
    } else {
        assert_ne!(rand.last(), 0, "{ctx}: the random fallback drew no RNG");
        cov.file_fallback += 1;
    }
    if level_file == SHADOW_FIXTURE {
        assert!(
            loaded,
            "{ctx}: the shadow fixture must load (else the line tests the fallback)"
        );
        let prev = cov.fixture_final.insert(shadow, t[6][..16].to_string());
        assert!(prev.is_none(), "{ctx}: duplicate shadow-fixture line");
    }
    cov.file_lines += 1;
}

#[test]
fn levelgen_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(format!("{ROOT}/rust/oracle-tests/golden/levelgen.txt"))
        .expect("read golden/levelgen.txt (regenerate with gen_levelgen_golden.sh)");
    let tc = load_tc();
    let mut cov = Coverage::default();
    for (i, line) in golden.lines().enumerate() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("gen") => check_gen(i + 1, &t, &tc, &mut cov),
            Some("file") => check_file(i + 1, &t, &tc, &mut cov),
            other => panic!("line {}: unknown record {other:?}", i + 1),
        }
    }

    // Coverage guards (design §10.7): the golden must exercise what it claims to.
    let sizes = [
        (504, 350),
        (600, 350),
        (64, 64),
        (128, 96),
        (101, 77),
        (2000, 72),
        (72, 1000),
    ];
    for (w, h) in sizes {
        for shadow in [false, true] {
            assert!(
                cov.combos.contains(&(w, h, shadow)),
                "golden lacks {w}x{h} shadow {shadow}"
            );
        }
    }
    assert!(
        cov.gen_lines >= 42,
        "expected >= 42 gen lines, got {}",
        cov.gen_lines
    );
    assert_eq!(cov.file_lines, 7, "file lines");
    assert!(
        cov.capped > 0,
        "no case hit the kMaxTries cap (level.cpp:137-158)"
    );
    assert!(
        cov.retried > 0,
        "no uncapped case rejected-then-placed a rock"
    );
    // 3 shipped-level lines + 2 shadow-fixture lines load; 2 missing-file lines fall back.
    assert!(
        cov.file_loaded >= 5,
        "file branch (no RNG) not exercised: {}",
        cov.file_loaded
    );
    assert!(
        cov.file_fallback >= 2,
        "missing-file fallback not exercised: {}",
        cov.file_fallback
    );

    // The shadow fixture: MakeShadow on the file branch must change the loaded map (the whole
    // point of the fixture), and the fixture is exactly the pre-shadow `gen 42 128 96` map.
    let on = cov
        .fixture_final
        .get(&true)
        .expect("golden lacks the shadow-fixture shadow=1 line");
    let off = cov
        .fixture_final
        .get(&false)
        .expect("golden lacks the shadow-fixture shadow=0 line");
    assert_ne!(
        on, off,
        "shadow fixture: shadow 1 == shadow 0 (file-branch MakeShadow check is vacuous)"
    );
    let (pre, shadowed) = cov
        .fixture_source
        .as_ref()
        .expect("golden lacks the gen 42 128 96 shadow=1 line");
    assert_eq!(
        off, pre,
        "shadow fixture shadow=0 != the gen 42 128 96 pre-shadow map"
    );
    assert_eq!(
        on, shadowed,
        "shadow fixture shadow=1 != the gen 42 128 96 MakeShadow map"
    );
}
