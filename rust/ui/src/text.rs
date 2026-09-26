//! Texts the C++ hardcodes in `Texts::Texts()` (`common.cpp:205-225`, design finding 3 — not
//! read from `tc.cfg`), the `text.cpp` time formatters, and `GetBasename(GetLeaf(..))`.

/// `Texts::game_modes` (`common.cpp:206-209`), indexed by `Settings::game_mode`.
pub const GAME_MODES: [&str; 4] = [
    "Kill'em All",
    "Game of Tag",
    "Holdazone",
    "Scales of Justice",
];
/// `Texts::onoff` (`common.cpp:211-212`).
pub const ONOFF: [&str; 2] = ["OFF", "ON"];

/// C++ `'0' + n` stored into a `char` (`text.cpp`): the digit for 0..=9, and the same byte
/// arithmetic (wrapping) outside it.
fn digit(n: i32) -> char {
    (b'0' as i32 + n) as u8 as char
}

/// `TimeToString(sec)` (`text.cpp:5-16`): `M M : S S` with the first digit `sec / 600`.
pub fn time_to_string(sec: i32) -> String {
    [
        digit(sec / 600),
        digit((sec % 600) / 60),
        ':',
        digit((sec % 60) / 10),
        digit(sec % 10),
    ]
    .iter()
    .collect()
}

/// `TimeToStringEx(ms, force_hours, force_minutes)` (`text.cpp:18-45`).
pub fn time_to_string_ex(ms: i32, force_hours: bool, force_minutes: bool) -> String {
    let mut ms = ms;
    let mut s = String::new();
    if ms >= 6_000_000 || force_hours {
        s.push(digit(ms / 6_000_000));
        ms %= 6_000_000;
    }
    if ms >= 60_000 || force_minutes {
        s.push(digit(ms / 600_000));
        ms %= 600_000;
        s.push(digit(ms / 60_000));
        ms %= 60_000;
        s.push(':');
    }
    s.push(digit(ms / 10_000));
    ms %= 10_000;
    s.push(digit(ms / 1000));
    ms %= 1000;
    s.push('.');
    s.push(digit(ms / 100));
    ms %= 100;
    s.push(digit(ms / 10));
    s
}

/// `TimeToStringFrames(frames)` (`text.cpp:47-49`): 14 ms per frame.
pub fn time_to_string_frames(frames: i32) -> String {
    time_to_string_ex(frames * 14, false, false)
}

/// `GetBasename(GetLeaf(path))` (`filesystem.cpp:38-54`).
pub fn leaf_basename(path: &str) -> &str {
    let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);
    leaf.rsplit_once('.').map_or(leaf, |(b, _)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_to_string_is_minutes_colon_seconds() {
        // text.cpp:5-16: '0' + sec/600, '0' + (sec%600)/60, ':', tens, units.
        assert_eq!(time_to_string(600), "10:00", "Settings(): time_to_lose");
        assert_eq!(time_to_string(3599), "59:59");
        assert_eq!(time_to_string(60), "01:00");
        assert_eq!(time_to_string(3600), "60:00");
        assert_eq!(time_to_string(45), "00:45");
    }

    #[test]
    fn time_to_string_frames_is_ms_at_14_per_frame() {
        // text.cpp:18-49: TimeToStringEx(frames * 14, false, false).
        assert_eq!(time_to_string_frames(70), "00.98");
        assert_eq!(
            time_to_string_frames(4286),
            "01:00.00",
            "60004 ms: minutes appear"
        );
        // An exact hour: the hours digit, then (ms == 0, no force) the minutes block is skipped.
        assert_eq!(time_to_string_ex(6_000_000, false, false), "100.00");
        assert_eq!(time_to_string_ex(1234, true, true), "000:01.23");
    }

    #[test]
    fn leaf_basename_is_getbasename_of_getleaf() {
        // filesystem.cpp:38-54: the leaf after the last '/' or '\', up to its last '.'.
        assert_eq!(leaf_basename("Levels/water_stage.lev"), "water_stage");
        assert_eq!(leaf_basename("C:\\a\\b.c.lev"), "b.c");
        assert_eq!(leaf_basename("noext"), "noext");
        assert_eq!(leaf_basename(""), "");
        assert_eq!(leaf_basename("data/Setups/liero.cfg"), "liero");
    }
}
