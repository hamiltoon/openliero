//! Step 4½d G1 — the menu-widget gate (design §6.1, done-when 1): every G1 script replayed
//! through `ui::menu` matches the REAL C++ `Menu` / behaviors / `SettingsMenu`
//! (`oracle_dump_menu`, `gen_menu_golden.sh`) line for line. `MENU_RUST_PPM_DIR=<dir>` writes the
//! Rust draws of a failing script as `<dir>/<script>/menu_NNNN.ppm` (the dumper's --ppm-dir
//! layout).

mod menu_common;

use menu_common as mc;

fn check(name: &str, want: &[String]) {
    let (got, shots) = mc::replay(name);
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if g != w {
            if let Ok(dir) = std::env::var("MENU_RUST_PPM_DIR") {
                let dir = std::path::Path::new(&dir).join(format!("menu_{name}"));
                std::fs::create_dir_all(&dir).unwrap();
                for (n, bmp) in &shots {
                    let mut data = format!("P6\n{} {}\n255\n", bmp.w, bmp.h).into_bytes();
                    for p in &bmp.pixels {
                        data.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
                    }
                    std::fs::write(dir.join(format!("menu_{n:04}.ppm")), data).unwrap();
                }
            }
            panic!("{name}: G1 line {i} differs\n  C++:  {w}\n  Rust: {g}");
        }
    }
    assert_eq!(got.len(), want.len(), "{name}: G1 line count");
}

#[test]
fn every_g1_script_matches_the_cpp_menu_line_for_line() {
    for name in mc::SCRIPTS {
        check(name, &mc::golden(name));
    }
}

#[test]
fn the_corpus_is_exactly_the_15_scripts() {
    let mut on_disk: Vec<String> = std::fs::read_dir(mc::GOLDEN)
        .unwrap()
        .filter_map(|e| {
            let f = e.unwrap().file_name().to_string_lossy().into_owned();
            Some(
                f.strip_prefix("menu_")?
                    .strip_suffix("_script.txt")?
                    .to_string(),
            )
        })
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = mc::SCRIPTS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(on_disk, want);
}

#[test]
#[should_panic(expected = "G1 line")]
fn the_gate_sees_a_one_field_change() {
    let mut want = mc::golden("nav_wrap");
    want[3].push_str(" x");
    check("nav_wrap", &want);
}
