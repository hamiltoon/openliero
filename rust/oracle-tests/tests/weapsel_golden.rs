//! Step 4½c T7 — MILESTONE (design §1 done-when 1, §6.3, §6.7): the sixteen weapon-selection
//! goldens line for line vs C++. Each committed `weapsel_<case>_scenario.txt` goes through the
//! real readers (`Scenario::parse`, `settings_from_toml` on its sidecar), `new_match`,
//! `weapsel_config` and `sim::weapsel`; `weapsel_common::golden_lines` prints what
//! `oracle_dump_weapsel` prints. The witness guard then proves the corpus reaches every branch.

mod weapsel_common;

use weapsel_common as wc;

fn run_case(c: &wc::Case) -> wc::Run {
    let scenario = wc::read_scenario(c.name);
    let settings = wc::read_settings(&scenario);
    wc::drive(c.name, &scenario, &settings).0
}

fn compare(name: &str, got: &[String], want: &[String]) {
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: golden line {k} (0 = init) differs");
    }
    assert_eq!(got.len(), want.len(), "{name}: line count");
}

#[test]
fn every_weapsel_golden_matches_line_for_line_and_the_corpus_reaches_every_branch() {
    let mut runs = Vec::new();
    for c in &wc::CASES {
        let run = run_case(c);
        compare(
            c.name,
            &wc::golden_lines(&run),
            &wc::golden_file_lines(c.name),
        );
        runs.push(run);
    }
    let missing: Vec<&str> = wc::witnesses(&runs)
        .into_iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| name)
        .collect();
    assert!(missing.is_empty(), "witnesses not reached: {missing:?}");
}

#[test]
#[should_panic(expected = "golden line")]
fn a_perturbed_golden_line_fails() {
    // Non-vacuity of `compare`: one changed character must fail.
    let c = wc::case("same_frame");
    let run = run_case(c);
    let mut want = wc::golden_file_lines(c.name);
    want[3] = want[3].replacen(' ', "  ", 1);
    compare(c.name, &wc::golden_lines(&run), &want);
}

#[test]
fn the_committed_corpus_is_the_sixteen_cases() {
    let mut on_disk: Vec<String> = std::fs::read_dir(wc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            n.strip_prefix("weapsel_")
                .and_then(|r| r.strip_suffix("_scenario.txt"))
                .map(String::from)
        })
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = wc::CASES.iter().map(|c| c.name.to_string()).collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
fn the_corpus_picks_and_tables_name_the_intended_weapons() {
    let o = wc::load_objects();
    let names = |c: &str, i: usize| {
        wc::case(c).players[i].weapons.map(|p| {
            if p == 0 {
                "-".to_string()
            } else {
                wc::weapon_name_of_pick(&o, p)
            }
        })
    };
    assert_eq!(
        names("disabled_saved_s1", 0),
        ["BAZOOKA", "DART", "BOUNCY LARPA", "ZIMM", "LASER"]
    );
    assert_eq!(
        names("disabled_saved_s1", 1),
        ["BIG NUKE", "BLASTER", "CANNON", "RIFLE", "SHOTGUN"]
    );
    assert_eq!(
        names("bot_keep", 1),
        ["-", "LASER", "BOUNCY LARPA", "BOUNCY MINE", "CANNON"]
    );
    let t = (wc::case("disabled_saved_s1").table)(&o);
    let off = |name: &str| t[wc::weapon_index(&o, name)] != 0;
    assert_eq!(
        ["BAZOOKA", "DART", "BOUNCY LARPA", "ZIMM", "LASER", "CANNON"].map(off),
        [true, true, false, true, true, false],
        "saved picks: four disabled + one enabled per player"
    );
    assert_eq!(t.iter().filter(|&&v| v != 0).count(), 12);
    assert!(
        t.contains(&1) && t.contains(&2),
        "a mix of bonus-only and banned"
    );
    let enabled = |c: &str| (wc::case(c).table)(&o).iter().filter(|&&v| v == 0).count();
    assert_eq!(
        [
            enabled("few_enabled"),
            enabled("five_enabled"),
            enabled("one_enabled"),
            enabled("bot_keep")
        ],
        [3, 5, 1, 37]
    );
}
