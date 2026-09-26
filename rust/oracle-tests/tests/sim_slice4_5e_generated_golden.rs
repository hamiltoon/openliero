//! Step 4½e-1 T7 — G3: four SETTINGS-DRIVEN matches on GENERATED levels that are not 504x350,
//! bit-exact vs C++ (plan T7; design §6.5). The first sim gate at other level sizes.
//!
//! Each committed scenario carries `generate <level_seed>` and a `settings <file>` sidecar. The C++
//! dumper built the level with the REAL `Level::GenerateFromSettings` over a `Rand` seeded
//! `level_seed`, read the sidecar with the real `Settings::FromToml` and started the worms in the
//! C++ LocalController state. Here the SAME committed files go through the real Rust readers —
//! `Scenario::parse` (the `generate` and `settings` directives), `settings_toml::settings_from_toml`
//! on the sidecar, `sim::levelgen::generate_from_settings` (`file = None`, a `Rand` seeded
//! `level_seed`) and `scenario::build::build_match`. Nothing is regenerated in memory. Every golden
//! row is asserted on the 11 hash columns (components first, master last) and on column 12,
//! `Game::IsGameOver()`, against `sim::game_over::is_game_over`.
//!
//! Cases (generator `examples/gen_slice4_5e1_sim.rs`): `small` 96x344 (Kill'em All, lives 3; the
//! plan's 96x64 makes C++ read past the level at the first spawn and segfault — the generator's
//! `CASES` doc), `odd` 333x211 (Scales, loading 37, blood 300, shadow), `tall` 160x1000 (Game of
//! Tag, time to lose 60), `banned` 1024x256 (max bonuses 20, 36 of 40 weapons banned). The
//! witness tests re-run every ledger claim of the scenario headers on the driven Rust state.

use std::path::Path;

use assets::level::LevelData;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use oracle_tests::scenario::Scenario;
use scenario::build::build_match;
use scenario::settings::{
    MatchConfig, Settings, GM_GAME_OF_TAG, GM_KILL_EM_ALL, GM_SCALES_OF_JUSTICE,
};
use scenario::settings_toml::settings_from_toml;
use sim::game_over::is_game_over;
use sim::hash::{hash_components, hash_game_state};
use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
use sim::state::{ControlState, SimState};
use sim_core::fixed::ftoi;
use sim_core::rng::Rand;

const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
/// The directory the scenario files live in — a `settings <file>` path is relative to it.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden");
/// C++ `LocalController`: 180 simulated frames after the game-over frame.
const POST_MORTEM: usize = 180;

fn read(rel: &str) -> String {
    let path = Path::new(GOLDEN).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

struct Row {
    tick: u32,
    hashes: [u32; 10], // master, rng, level, worm0, worm1, bob, bon, sob, nob, wob
    game_over: u32,
}

/// Parse a 12-column settings-path golden; row `k` must carry tick `k`.
fn parse_golden(text: &str) -> Vec<Row> {
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(cols.len(), 12, "settings-path golden lines have 12 columns");
            let mut hashes = [0u32; 10];
            for (i, c) in cols[1..11].iter().enumerate() {
                hashes[i] = u32::from_str_radix(c, 16).expect("hex column");
            }
            let game_over: u32 = cols[11].parse().expect("game-over column");
            assert!(game_over <= 1, "IsGameOver column is 0/1, got {game_over}");
            Row {
                tick: cols[0].parse().expect("tick"),
                hashes,
                game_over,
            }
        })
        .collect();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(r.tick, k as u32, "golden row {k} carries tick {}", r.tick);
    }
    rows
}

fn check(state: &SimState, row: &Row) {
    let c = hash_components(state);
    let got = [
        hash_game_state(state),
        c.rng,
        c.level,
        c.worms[0],
        c.worms[1],
        c.bobjects,
        c.bonuses,
        c.sobjects,
        c.nobjects,
        c.wobjects,
    ];
    let names = [
        "master", "rng", "level", "worm0", "worm1", "bob", "bon", "sob", "nob", "wob",
    ];
    // Components first, master last, so a divergence localises.
    for i in (1..10).chain(0..1) {
        assert_eq!(
            got[i], row.hashes[i],
            "tick {}: {}: got {:08x} want {:08x}",
            row.tick, names[i], got[i], row.hashes[i]
        );
    }
    assert_eq!(
        is_game_over(state) as u32,
        row.game_over,
        "tick {}: IsGameOver",
        row.tick
    );
}

struct Case {
    name: &'static str,
    scenario: Scenario,
    cfg: MatchConfig,
    level: LevelData,
}

/// `Level::GenerateFromSettings` over a `Rand` seeded `level_seed` (the dumper's `generate`).
fn generate(s: &Settings, level_seed: u32) -> LevelData {
    let tc = TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap()).unwrap();
    let tga = Tga::load(&std::fs::read(format!("{TC_ROOT}/sprites/large.tga")).unwrap()).unwrap();
    let large = SpriteSet::from_tga(&tga, 16, 16, 110).unwrap();
    let assets = LevelGenAssets {
        large_sprites: &large,
        textures: &tc.textures,
        material_flags: &tc.materials,
    };
    let params = LevelGenParams {
        random_level: s.random_level,
        random_map_width: s.random_map_width,
        random_map_height: s.random_map_height,
        shadow: s.shadow,
    };
    let mut rand = Rand::new();
    rand.seed(level_seed);
    generate_from_settings(&assets, &params, None, &mut rand)
}

/// Parse the COMMITTED `sim_slice4_5e_<name>_scenario.txt`, read the sidecar its `settings`
/// directive names, and generate the level its `generate` directive seeds.
fn case(name: &'static str, level_seed: u32, dims: (i32, i32)) -> Case {
    let scenario = Scenario::parse(&read(&format!("sim_slice4_5e_{name}_scenario.txt")))
        .unwrap_or_else(|e| panic!("{name}: scenario parses: {e}"));
    assert_eq!(
        scenario.settings.as_deref(),
        Some(format!("sim_slice4_5e_{name}_setup.cfg").as_str()),
        "{name}: the directive names the sidecar"
    );
    assert_eq!(scenario.generate(), Some(level_seed), "{name}: generate");
    assert_eq!(
        scenario.level, "",
        "{name}: a generate scenario has no level path"
    );
    assert!(scenario.worms.is_empty(), "worms come from the setup");
    let rel = scenario.settings.clone().unwrap();
    let settings = settings_from_toml(&read(&rel))
        .unwrap_or_else(|e| panic!("{name}: setup {rel} parses: {e:?}"));
    assert!(
        settings.random_level,
        "{name}: `generate` needs randomLevel = true"
    );
    assert_eq!(
        (settings.random_map_width, settings.random_map_height),
        dims,
        "{name}: map size"
    );
    let level = generate(&settings, level_seed);
    assert_eq!((level.width, level.height), dims, "{name}: generated size");
    let cfg = MatchConfig {
        settings,
        seed: scenario.seed,
    };
    Case {
        name,
        scenario,
        cfg,
        level,
    }
}

fn small() -> Case {
    case("small", 9601, (96, 344))
}
fn odd() -> Case {
    case("odd", 33321, (333, 211))
}
fn tall() -> Case {
    case("tall", 1601000, (160, 1000))
}
fn banned() -> Case {
    case("banned", 1024256, (1024, 256))
}

fn golden(c: &Case) -> Vec<Row> {
    let g = parse_golden(&read(&format!("sim_slice4_5e_{}.txt", c.name)));
    assert_eq!(
        g.len() as u32,
        c.scenario.ticks + 1,
        "{}: golden rows 0..=ticks",
        c.name
    );
    g
}

/// What the ledger claims, re-derived from the genuinely driven Rust state (the generator's
/// ledger, same definitions).
#[derive(Default)]
struct Witness {
    game_over_tick: Option<u32>,
    game_over_held: bool,
    spawns: u32,
    deaths: Vec<(u32, usize)>,
    respawns: Vec<(u32, usize)>,
    /// `(tick, x, y)`: a wobject/nobject slot live at t-1 and free at t, at a t-1 position
    /// outside the level rectangle.
    freed_outside: Vec<(u32, i32, i32)>,
    reload_started: bool,
    peak_bobjects: usize,
    /// `(tick, from, to)`: one worm's `lives * health_setting + health` fell and the other's rose.
    transfers: Vec<(u32, usize, usize)>,
    /// `(tick, weapon index)` of every newly spawned weapon bonus.
    weapon_bonuses: Vec<(u32, usize)>,
    angle_ok: bool,
    masters: Vec<u32>,
}

type Slots = Vec<Option<(i32, i32)>>;

fn object_slots(st: &SimState) -> [Slots; 2] {
    let w = (0..st.wobjects.capacity())
        .map(|i| st.wobjects.get(i).map(|o| (ftoi(o.pos.x), ftoi(o.pos.y))))
        .collect();
    let n = (0..st.nobjects.capacity())
        .map(|i| st.nobjects.get(i).map(|o| (ftoi(o.pos.x), ftoi(o.pos.y))))
        .collect();
    [w, n]
}

fn bonus_slots(st: &SimState) -> Vec<Option<(i32, i32)>> {
    (0..st.bonuses.capacity())
        .map(|i| st.bonuses.get(i).map(|b| (b.frame, b.weapon)))
        .collect()
}

fn total(st: &SimState, i: usize) -> i64 {
    i64::from(st.worms[i].lives) * i64::from(st.settings_health) + i64::from(st.worms[i].health)
}

fn angles_ok(st: &SimState) -> bool {
    st.worms
        .iter()
        .all(|w| (0..128).contains(&ftoi(w.aiming_angle)))
}

fn drive(c: &Case, cfg: &MatchConfig, golden: Option<&[Row]>) -> Witness {
    let mut st = build_match(Path::new(TC_ROOT), cfg, &c.level)
        .expect("builds")
        .state;
    let (lw, lh) = (c.level.width, c.level.height);
    let mut w = Witness {
        game_over_held: true,
        angle_ok: angles_ok(&st),
        ..Witness::default()
    };
    if let Some(g) = golden {
        check(&st, &g[0]);
    }
    w.masters.push(hash_game_state(&st));
    let mut died = [false; 2];
    let mut prev_vis = [st.worms[0].visible, st.worms[1].visible];
    let mut prev_tot = [total(&st, 0), total(&st, 1)];
    let mut prev_obj = object_slots(&st);
    let mut prev_bon = bonus_slots(&st);
    for k in 1..=c.scenario.ticks {
        st.process_frame(&[
            ControlState::unpack(c.scenario.input(k - 1, 0)),
            ControlState::unpack(c.scenario.input(k - 1, 1)),
        ]);
        if let Some(g) = golden {
            check(&st, &g[k as usize]);
        }
        w.masters.push(hash_game_state(&st));
        w.angle_ok &= angles_ok(&st);
        for i in 0..2 {
            let vis = st.worms[i].visible;
            if prev_vis[i] && !vis {
                w.deaths.push((k, i));
                died[i] = true;
            }
            if !prev_vis[i] && vis {
                w.spawns += 1;
                if died[i] {
                    w.respawns.push((k, i));
                }
            }
            prev_vis[i] = vis;
            w.reload_started |= st.worms[i].weapons.iter().any(|x| x.loading_left > 0);
        }
        let tot = [total(&st, 0), total(&st, 1)];
        for i in 0..2 {
            if tot[i] < prev_tot[i] && tot[1 - i] > prev_tot[1 - i] {
                w.transfers.push((k, i, 1 - i));
            }
        }
        prev_tot = tot;
        let obj = object_slots(&st);
        for (before, after) in prev_obj.iter().zip(obj.iter()) {
            for (a, b) in before.iter().zip(after.iter()) {
                if let (Some(p), None) = (a, b) {
                    if p.0 < 0 || p.1 < 0 || p.0 >= lw || p.1 >= lh {
                        w.freed_outside.push((k, p.0, p.1));
                    }
                }
            }
        }
        prev_obj = obj;
        let bon = bonus_slots(&st);
        for (a, b) in prev_bon.iter().zip(bon.iter()) {
            if let (None, Some((0, weapon))) = (a, b) {
                w.weapon_bonuses.push((k, *weapon as usize));
            }
        }
        prev_bon = bon;
        w.peak_bobjects = w.peak_bobjects.max(st.bobjects.len());
        if w.game_over_tick.is_none() {
            if is_game_over(&st) {
                w.game_over_tick = Some(k);
            }
        } else {
            w.game_over_held &= is_game_over(&st);
        }
    }
    w
}

/// The level-independent guards every case shares: both worms spawned, the angle guard held,
/// and the settings path is the one the sidecar says.
fn assert_common(c: &Case, w: &Witness) {
    assert!(w.spawns >= 2, "{}: both worms spawn", c.name);
    assert!(
        w.angle_ok,
        "{}: every aiming_angle stays in 0..128 (no cossin_table[128] read)",
        c.name
    );
    assert!(w.reload_started, "{}: a reload runs", c.name);
}

/// A never-over case: C++ column 12 is 0 on every row (and `check` has matched Rust to it).
fn assert_never_over(c: &Case, golden: &[Row], w: &Witness) {
    assert!(
        golden.iter().all(|r| r.game_over == 0),
        "{}: C++ never over",
        c.name
    );
    assert_eq!(w.game_over_tick, None, "{}: Rust never over", c.name);
}

#[test]
fn small_matches_cpp() {
    let c = small();
    let s = &c.cfg.settings;
    assert_eq!((s.game_mode, s.lives), (GM_KILL_EM_ALL, 3));
    assert!(
        c.level.width < 158,
        "narrower than a viewport: the negative max_x clamp"
    );
    let g = golden(&c);
    let w = drive(&c, &c.cfg, Some(&g));
    assert_common(&c, &w);
    assert_never_over(&c, &g, &w);
    assert_eq!(w.deaths, vec![(397, 0)], "the ledger's death");
    assert_eq!(w.respawns, vec![(594, 0)], "the ledger's respawn search");
    assert_eq!(w.spawns, 3);
    assert_eq!(w.peak_bobjects, 373);
    assert_eq!(
        w.freed_outside.len(),
        16,
        "objects freed after leaving the level"
    );
    assert_eq!(
        w.freed_outside[0],
        (312, -2, 175),
        "the first one, off the left edge"
    );
}

#[test]
fn odd_matches_cpp() {
    let c = odd();
    let s = &c.cfg.settings;
    assert_eq!(
        (s.game_mode, s.loading_time, s.blood, s.shadow),
        (GM_SCALES_OF_JUSTICE, 37, 300, true)
    );
    assert!(
        c.level.width % 8 != 0 && c.level.height % 8 != 0,
        "non-multiples of 8"
    );
    let g = golden(&c);
    let w = drive(&c, &c.cfg, Some(&g));
    assert_common(&c, &w);
    assert_never_over(&c, &g, &w);
    assert!(w.peak_bobjects >= 50, "blood 300: >= 50 live bobjects");
    assert_eq!(w.peak_bobjects, 371);
    assert_eq!(w.transfers.len(), 31, "Scales health transfers");
    assert_eq!(w.transfers[0], (222, 0, 1), "the first transfer");
    assert_eq!(w.deaths, vec![(435, 1)]);
    assert_eq!(w.respawns, vec![(585, 1)]);
}

#[test]
fn tall_matches_cpp() {
    let c = tall();
    let s = &c.cfg.settings;
    assert_eq!((s.game_mode, s.time_to_lose), (GM_GAME_OF_TAG, 60));
    let g = golden(&c);
    let w = drive(&c, &c.cfg, Some(&g));
    assert_common(&c, &w);
    let flip = g
        .iter()
        .position(|r| r.game_over == 1)
        .expect("C++ game over");
    assert_eq!(flip, 4760, "C++ flip tick");
    assert_eq!(w.game_over_tick, Some(4760), "Rust flips on the same tick");
    assert!(
        g[flip..].iter().all(|r| r.game_over == 1) && w.game_over_held,
        "IsGameOver flips exactly once and stays 1"
    );
    assert!(g.len() - flip >= 200, ">= 200 rows after the flip");
    assert!(g.len() - flip > POST_MORTEM, "the post-mortem is recorded");
    assert_eq!(w.deaths, vec![(396, 0)]);
    assert_eq!(w.respawns, vec![(600, 0)]);
}

#[test]
fn banned_matches_cpp() {
    let c = banned();
    let s = &c.cfg.settings;
    assert_eq!(s.max_bonuses, 20);
    let kept: Vec<usize> = (0..40).filter(|&i| s.weap_table[i] != 2).collect();
    assert_eq!(kept.len(), 4, "36 of 40 weapons banned");
    assert!(s.weap_table.iter().all(|&v| v == 0 || v == 2));
    let g = golden(&c);
    let w = drive(&c, &c.cfg, Some(&g));
    assert_common(&c, &w);
    assert_never_over(&c, &g, &w);
    let ticks: Vec<u32> = w.weapon_bonuses.iter().map(|b| b.0).collect();
    assert_eq!(ticks, vec![554, 600, 1294], ">= 3 weapon bonuses spawned");
    for (t, weapon) in &w.weapon_bonuses {
        assert!(
            kept.contains(weapon),
            "t{t}: the redraw loop ends on a kept weapon (got {weapon})"
        );
    }
    let objects = assets::object::Objects::load(
        &TcConfig::load(&std::fs::read(format!("{TC_ROOT}/tc.cfg")).unwrap())
            .unwrap()
            .types,
        |sub, id| std::fs::read(format!("{TC_ROOT}/{sub}/{id}.cfg")),
    )
    .unwrap();
    let names: Vec<&str> = w
        .weapon_bonuses
        .iter()
        .map(|b| objects.weapons[b.1].name.as_str())
        .collect();
    assert_eq!(
        names,
        ["UZI", "UZI", "SHOTGUN"],
        "the ledger's bonus weapons"
    );
    // The players' picks are untouched by the ban (C++ InitWeapons reads them as they are).
    assert!(
        s.worm_settings[..2]
            .iter()
            .flat_map(|ws| ws.weapons)
            .any(|m| {
                let order = sim::weapsel::weap_order(&objects.weapons);
                s.weap_table[order[m as usize - 1]] == 2
            }),
        "some pick names a banned weapon"
    );
}

#[test]
fn every_case_is_internally_deterministic() {
    for c in [small(), odd(), tall(), banned()] {
        assert_eq!(
            drive(&c, &c.cfg, None).masters,
            drive(&c, &c.cfg, None).masters,
            "{}",
            c.name
        );
    }
}

/// `scenario::load` (the tick-0 path) refuses a `generate` scenario: it refuses every `settings`
/// scenario — here the committed `small` scenario itself.
#[test]
#[should_panic(expected = "settings")]
fn scenario_load_refuses_a_committed_generate_scenario() {
    let s = Scenario::parse(&read("sim_slice4_5e_small_scenario.txt")).expect("parses");
    assert!(s.generate().is_some());
    let _ = scenario::load(Path::new(TC_ROOT), &s);
}
