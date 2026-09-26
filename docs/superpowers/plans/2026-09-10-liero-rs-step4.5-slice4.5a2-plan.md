# Step 4½, Slice 4½a-2 — settings persistence: TOML writer, byte gate, `UpdateHash`, storage, HUD fix: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Test-first**: write the failing test, run it and SEE it fail (RED), then make it pass (GREEN).

**Goal:** Rust saves settings with exactly the bytes C++ saves. That means a hand port of the toml++ 3.4 formatter subset the C++ archive reaches, proven by Hard gate 5: Rust load + save equals C++ load + save on the shipped setups and profiles, the 4½a-1 sidecars and a synthetic edge corpus, and every C++-saved file round-trips through Rust. `Settings::UpdateHash` / `WormSettings::UpdateHash` are ported and matched against C++ vectors. The slice also adds a `ConfigStore` seam with a native merged-read store mirroring `paths::Resolve`, centralises `TC_ROOT`/`DATA_ROOT`, and makes the live `game` binary draw the HUD.

**Architecture:** A new C++ oracle, `oracle_dump_settings`, runs the REAL `Settings::load`/`save`, `WormSettings::LoadProfile`/`SaveProfile` and both `UpdateHash`es over the entries of one corpus manifest (`rust/oracle-tests/golden/settings/corpus.txt`) and commits what C++ writes. On the Rust side, a private `scenario::toml_fmt` ports toml++'s `toml_formatter` (sorted keys, table layout, the 120-column array rule, literal-vs-basic string quoting). The writer functions (`settings_to_toml`, `worm_settings_to_toml`, `gameplay_toml`) and the hashes (`update_hash`, `worm_update_hash` via `twox-hash`) join the 4½a-1 reader in `settings_toml.rs`. `oracle-tests/tests/settings_toml_golden.rs` reads the same manifest and asserts G5a/G5b/G5c byte-for-byte. `scenario::paths` holds the two roots. `scenario::storage` holds `ConfigStore`, `NativeStore` (user dir over `data/`, writes to the user dir only, `ShadowsSystem` refusals, an SDL-compatible `pref_path`), `MemoryStore`, and `load_setup`/`save_setup`. A Bevy-free `game::hud_mode` decides the HUD flags the binary draws with. The binary does no config I/O in this slice (design §9.3.6).

**Tech Stack:** Rust 2021 (`scenario`, `oracle-tests`, `shot`) and 2024 (`game`); `toml` 0.8 (the reader, already locked); **new** `twox-hash =2.1.3` in `scenario` only (XXH3-64, `default-features = false`); C++ `src/tools/oracle_dump/settings_dump.cpp` (preset `macos-arm64`; toml++ 3.4.0, xxHash and cereal from vcpkg); clang-format 22.1.0 via `uvx`.

**Spec:** `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` (cited **design §N**). This plan implements **4½a-2** — design §9.2 as refined by the plan-time decisions in **design §9.3**. Design §3.1 (what C++ writes), §3.3 (writer), §3.4 (the gate), §3.5 (storage) and §8 (HUD) are the requirements; 4½a-1 (model, reader, builder) has landed on this branch (HEAD `03de202`).

## Global Constraints

- Worktree `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5`, branch `liero-rs-step-4-5`. Never `cd`; use `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 …` and absolute paths. Every cargo command passes `--manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml`.
- Bash hygiene: one simple command per call; no `&&`, `;`, `$VAR`, heredocs, `>`/`>>` redirection, `find -exec`. Create and edit files with the editor tools. No sub-subagents. (The committed `gen_settings_golden.sh` may use shell features internally — it is run as one `bash <abs path>` call.)
- Determinism and the sim are untouched: nothing under `rust/sim/` or `rust/sim-core/` changes (`git -C … diff --stat 03de202 -- rust/sim rust/sim-core` stays empty); no `SimState` field, no sim hash, no scenario-grammar change.
- Every prior golden stays byte-identical: `git -C … diff --name-status 03de202 -- rust/oracle-tests/golden` may list only `A` (added) lines, and those only under `rust/oracle-tests/golden/settings/`.
- `sim-core` stays dependency-free; `sim` gains no dependency; Bevy stays confined to `game`. The ONLY new crate is `twox-hash =2.1.3` (`default-features = false, features = ["xxhash3_64"]`) in `rust/scenario/Cargo.toml` — pure Rust, no dependencies in that configuration, wasm-safe (design §3.4, §9.3.5).
- C++ changes are confined to `src/tools/oracle_dump/settings_dump.cpp` (new) plus two lines inside the `OPENLIERO_BUILD_ORACLE_DUMP` block of `CMakeLists.txt`. The C++ file must pass `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file <abs file>`.
- The byte gate's truth is C++ output, never a reading of the toml++ source: when a Rust byte disagrees with a committed C++ golden, Rust changes. Goldens are only ever rewritten by `gen_settings_golden.sh`.
- rustfmt only files this plan CREATES (`rustfmt --edition 2021 <abs file>`, `--edition 2024` for `game`); never run rustfmt on an existing file or on a `lib.rs`/`main.rs` (it recurses) — hand-format edits to the surrounding style. `rustfmt --check` on an existing file is allowed as a read-only guide: apply by hand only the hunks that fall inside code you added.
- wasm persistence (`LocalStorageStore`) is 4½h: the `ConfigStore` seam is designed for it, only native + in-memory stores are implemented. The `game` binary performs no config file I/O in this slice (design §9.3.6).
- A sibling slice (4½c-0) is being planned/executed in the same worktree: never touch files matching `*slice4.5c0*` or `*slice4_5c0*`; re-read `CMakeLists.txt`, PROGRESS and the overview immediately before editing them.
- Commits use the globally-configured identity (john.alm.martensson@pm.me); do not override it. Every commit message ends with two trailer paragraphs passed as extra `-m` arguments: `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, naming the model that actually did the task's work (the commit lines below name each task's tier model — `Claude Opus 5` / `Claude Sonnet 5` — change it if a different model executes the task), and `Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz` (use the executing session's URL if it is a different session). Never write "Generated with Claude Code" anywhere.
- Do NOT push and do NOT open a PR — the controller owns push + PR.
- Literal-escape hazard for editors: the files below contain backslash escapes. The TOML inputs deliberately use TOML's 8-digit `\U000000XX` escapes, and Rust sources use `\u{..}` for characters and `"\\u0001"`-style double backslashes for escape TEXT. Copy them verbatim, and after creating each input run `sed -n 1,40l <abs file>` to confirm that no raw control byte (shown as `\001`, `\177`, …) slipped in.

## Model tiers

- **[Opus]:** T0 (C++ oracle + corpus design), T1 (the formatter port — fidelity), T6 (MILESTONE), T7 (final re-diff + broad review) — and every reviewer.
- **[Sonnet]:** T2 (gate harness), T3 (hash), T4 (paths + storage), T5 (HUD) — mechanical, fully specified.

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `src/tools/oracle_dump/settings_dump.cpp` (create) | T0 | `oracle_dump_settings`: C++ load + save + `UpdateHash` over the corpus, self-checked |
| `CMakeLists.txt` (modify, oracle block `:372-394`) | T0 | `add_executable` + `target_link_libraries(… PRIVATE game cereal::cereal)` |
| `rust/oracle-tests/gen_settings_golden.sh` (create) | T0 | builds the tool, regenerates `golden/settings/` outputs |
| `rust/oracle-tests/golden/settings/corpus.txt` (create) | T0 | the corpus manifest — read by the C++ tool AND the Rust test |
| `rust/oracle-tests/golden/settings/in/*.{cfg,toml}` (create, 6) | T0 | synthetic quoting / edge / missing-table / array-valued inputs |
| `rust/oracle-tests/golden/settings/{<id>.cfg,<id>.gameplay.toml,<id>.toml,hashes.txt}` (generated, 32) | T0 | what C++ saves; the `UpdateHash` vectors |
| `rust/scenario/src/toml_fmt.rs` (create) | T1 | private port of toml++ `toml_formatter` (layout, arrays, string quoting) |
| `rust/scenario/src/settings_toml.rs` (modify) | T1, T3 | + `settings_to_toml`, `worm_settings_to_toml`, `gameplay_toml`; + `update_hash`, `worm_update_hash` |
| `rust/scenario/src/lib.rs` (modify) | T1, T4 | `mod toml_fmt;`, `pub mod paths;`, `pub mod storage;` |
| `rust/scenario/Cargo.toml`, `rust/Cargo.lock` (modify) | T3 | `twox-hash = "=2.1.3"` |
| `rust/oracle-tests/tests/settings_toml_golden.rs` (create) | T2, T3, T6 | G5a + G5b (T2), G5c (T3), milestone intent guards (T6) |
| `rust/scenario/src/paths.rs` (create) | T4 | `DATA_ROOT`, `TC_ROOT` |
| `rust/scenario/src/storage.rs` (create) | T4 | `ConfigStore`, `NativeStore`, `MemoryStore`, `pref_path(_for)`, `PrefOs`, `is_reserved`, `load_setup`, `save_setup` |
| `rust/game/src/main.rs` (modify) | T4, T5 | use `scenario::paths::TC_ROOT`; `Demo.hud` + apply it in `render_and_upload` |
| `rust/shot/src/lib.rs` (modify `:392-397`) | T4 | default TC root from `scenario::paths::TC_ROOT` |
| `rust/game/src/hud_mode.rs` (create), `rust/game/src/lib.rs` (modify) | T5 | `HudFlags`, `hud_flags` |
| `docs/superpowers/liero-rs-PROGRESS.md`, the overview, the design (modify) | T7 | status |

## Task dependency map

```
T0 (C++ oracle + corpus + goldens) ──> T1 (toml_fmt + writer) ──> T2 (G5a/G5b) ──> T3 (UpdateHash, G5c) ──> T6 MILESTONE ──> T7
                                        └──> T4 (paths + storage + TC_ROOT) ──> T5 (HUD) ───────────────────┘
```

T4 needs T1's `settings_to_toml`; T5 needs T4's `scenario::paths::TC_ROOT`, and both edit `game/src/main.rs`, so they run in that order.

---

### Task 0: The C++ oracle `oracle_dump_settings`, the corpus, the goldens  [Opus]

**Files:**
- Create: `rust/oracle-tests/golden/settings/corpus.txt` and the six inputs `rust/oracle-tests/golden/settings/in/{quoting.cfg,edge.cfg,missing_tables.cfg,array_player.cfg,quoting_profile.toml,edge_profile.toml}`
- Create: `rust/oracle-tests/gen_settings_golden.sh`
- Create: `src/tools/oracle_dump/settings_dump.cpp`
- Modify: `CMakeLists.txt` — inside `if(OPENLIERO_BUILD_ORACLE_DUMP)` (`:372-394`), directly before its `endif()`
- Generated (committed): `rust/oracle-tests/golden/settings/` — `<id>.cfg` + `<id>.gameplay.toml` for the 10 setup entries, `<id>.toml` for the 11 profile entries, `hashes.txt` (32 files)

**Interfaces:**
- Produces (used by T1, T2, T3, T6):
  - `corpus.txt` grammar: one entry per line, `#` lines and blank lines skipped; `<kind> <id> [<path>]` where `<kind>` ∈ `default-setup`, `default-profile` (no path), `setup`, `profile` (path = the rest of the line after the second space, may contain spaces, relative to the repo root). 21 entries: 1 `default-setup`, 1 `default-profile`, 9 `setup`, 10 `profile`.
  - Per setup entry `<id>`: `golden/settings/<id>.cfg` (= `Settings::save` after `Settings::load` over a fresh `Settings`, or `Settings()` itself) and `golden/settings/<id>.gameplay.toml` (= the `SerializeGameplay` bytes `Settings::UpdateHash` hashes).
  - Per profile entry `<id>`: `golden/settings/<id>.toml` (= `SaveProfile` after `LoadProfile` over a bare `WormSettings()`, or `WormSettings()` itself).
  - `golden/settings/hashes.txt`: line 1 `xxh3-empty - <XXH3_64 of zero bytes, %016llx>`, then `<kind> <id> <hash %016llx>` per entry in corpus order (`Settings::UpdateHash()` for setups, `WormSettings::UpdateHash()` for profiles).

Why: design §3.4 / §9.3.1. The shipped files are legacy formats C++ itself rewrites, so the gate compares Rust against what **C++ writes for the same input**. One manifest read by both sides keeps the corpus in one place; the tool's self-checks guarantee each hash really is XXH3 of the written bytes and that C++ round-trips its own output (G5b's premise).

- [ ] **Step 1: Write the corpus (the oracle's input)**

Create `rust/oracle-tests/golden/settings/corpus.txt`:

```
# Step 4½a-2 settings byte-gate corpus (design §3.4, §9.3.1). Read by BOTH
# src/tools/oracle_dump/settings_dump.cpp (gen_settings_golden.sh) and
# rust/oracle-tests/tests/settings_toml_golden.rs. One entry per line: <kind> <id> [<path>];
# <path> is the rest of the line (it may contain spaces), relative to the repo root.
# Outputs: <id>.cfg + <id>.gameplay.toml per setup, <id>.toml per profile, hashes.txt.
default-setup defaults
default-profile default_profile
setup liero data/Setups/liero.cfg
setup orbmit data/Setups/orbmit.cfg
setup sidecar_killemall rust/oracle-tests/golden/sim_slice4_5a_killemall_setup.cfg
setup sidecar_scales rust/oracle-tests/golden/sim_slice4_5a_scales_setup.cfg
setup sidecar_gametag rust/oracle-tests/golden/sim_slice4_5a_gametag_setup.cfg
setup quoting rust/oracle-tests/golden/settings/in/quoting.cfg
setup edge rust/oracle-tests/golden/settings/in/edge.cfg
setup missing_tables rust/oracle-tests/golden/settings/in/missing_tables.cfg
setup array_player rust/oracle-tests/golden/settings/in/array_player.cfg
profile ai_l data/Profiles/AI (L).toml
profile ai_r data/Profiles/AI (R).toml
profile joystick0 data/Profiles/Joystick0.toml
profile joystick1 data/Profiles/Joystick1.toml
profile lefty_l data/Profiles/Lefty (L).toml
profile lefty_r data/Profiles/Lefty (R).toml
profile righty_l data/Profiles/Righty (L).toml
profile righty_r data/Profiles/Righty (R).toml
profile quoting_profile rust/oracle-tests/golden/settings/in/quoting_profile.toml
profile edge_profile rust/oracle-tests/golden/settings/in/edge_profile.toml
```

Create `rust/oracle-tests/golden/settings/in/quoting.cfg` (one string per toml++ quoting rule, design §3.1 / §9.3.3; the input is written with TOML escapes so it holds no raw control byte):

```toml
# Step 4½a-2 synthetic byte-gate input: one string per toml++ quoting rule (design §3.1).
# Escapes are TOML's 8-digit \U form, so this file holds no raw control byte.
[settings]
tc = "mix \"q\" 'a' \\x"
levelFile = "del\U0000007F"

[player1]
rgbDepth = 8
name = "O'Brien"
gamepadName = "say \"hi\""
gamepadSerial = "C:\\pads\\1"

[player2]
rgbDepth = 8
name = "tab\there"
gamepadName = "Zo\U000000E9"
gamepadSerial = "a\U00002028b"

[network_player]
rgbDepth = 8
name = "bell\U00000001esc\U0000001B"
gamepadName = "two\nlines"
gamepadSerial = "it's\nmulti"
```

Create `rust/oracle-tests/golden/settings/in/edge.cfg`:

```toml
# Step 4½a-2 synthetic byte-gate input: TomlInputArchive edge cases (design §3.2) - wrong
# types keep the prior value, integers narrow like static_cast, arrays are positional
# (short / long / empty), rgbDepth 7 expands, 8-bit values clamp, and the missing
# [player2] still gets its DEFAULT rgb 6-bit expanded (the C++ quirk).
[settings]
version = 3
lives = 4294967311
gameMode = -1
selectBotWeapons = -1
maxBonuses = 9223372036854775807
loadingTime = -7
blood = "lots"
shadow = 1
aiTraces = "yes"
weapTable = [2, "x", 1]

[player1]
name = 42
health = -2147483649
rgbDepth = 7
rgb = [63, 64, 1, 99]
weapons = [5, 6, 7, 8, 9, 10, 11]
controls = [1, 2]
controlsEx = [4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295, 4294967295]

[network_player]
rgbDepth = 8
rgb = [300, -5, 70]
weapons = []
```

Create `rust/oracle-tests/golden/settings/in/missing_tables.cfg`:

```toml
# Step 4½a-2 synthetic byte-gate input: no worm tables at all (every DEFAULT rgb is then
# 6-bit expanded - the C++ quirk, design §3.2) and a 41-entry weapTable whose extra last
# entry is ignored. weapTable[i] = i % 3 for i in 0..40.
[settings]
lives = 3
weapTable = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 2]
```

Create `rust/oracle-tests/golden/settings/in/array_player.cfg`:

```toml
# Step 4½a-2 synthetic byte-gate input: ARRAY-valued `settings` and `player1` (design §3.2,
# corrected 2026-09-10). TomlInputArchive's Lookup on an array frame ignores the key and
# reads slot `index`, so fields are consumed BY POSITION in serialization order. This file
# confirms the Rust reader's positional reading against the real C++.
settings = [99, true, false]
player1 = ["Pos", 55, 7, false, 3, 1, "g", "s", 8, [1, 2, 3], [9]]
```

Create `rust/oracle-tests/golden/settings/in/quoting_profile.toml`:

```toml
# Step 4½a-2 synthetic byte-gate profile: the quoting rules quoting.cfg has no room for -
# a carriage return, newline + control (multi-line BASIC), backslash + newline (multi-line
# literal).
name = "cr\rhere"
gamepadName = "x\ny\U00000002"
gamepadSerial = "back\\slash\nnext"
rgbDepth = 8
rgb = [1, 2, 3]
```

Create `rust/oracle-tests/golden/settings/in/edge_profile.toml`:

```toml
# Step 4½a-2 synthetic byte-gate profile: LoadProfile edge cases - the colour is restored,
# wrong types keep, arrays are short / long, no rgbDepth (6-bit), u32 narrowing, and a name
# mixing an escaped non-ASCII vertical space (U+0085) with raw UTF-8.
name = "nel\U00000085Zo\U000000E9"
health = "x"
controller = 4294967297
color = 99
randomName = 0
weapons = [3]
controls = [1, 2, 3, 4, 5, 6, 7, 8, 9]
gamepadControls = [-1]
rgb = [63, 0, 32]
```

Run (once per input): `sed -n 1,40l /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/settings/in/quoting.cfg` (likewise for the other five).
Expected: every line ends in `$`; the escapes appear as the literal text `\U0000007F`, `\t`, `\n`, `\\` …; NO octal byte like `\001`, `\033` or `\177` and no raw `é` (a decoded escape means the editor rewrote it — fix that line).

- [ ] **Step 2: Write the generator script and see it fail**

Create `rust/oracle-tests/gen_settings_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates rust/oracle-tests/golden/settings/{*.cfg,*.toml,hashes.txt} - the C++ side of
# the Step-4½ slice-4½a-2 settings byte gate (design §3.4, §9.3.1). oracle_dump_settings runs
# the REAL C++ Settings::load/save, WormSettings::LoadProfile/SaveProfile and UpdateHash over
# every entry of golden/settings/corpus.txt (src/tools/oracle_dump/settings_dump.cpp). The
# hand-written inputs (corpus.txt, in/) are never touched. Needs the full C++ build (links the
# `game` target), so this is a LOCAL/MANUAL step - NOT run in the lightweight rust.yml CI.
# Override PRESET for other platforms (e.g. linux-x64). cd's to ROOT, so any cwd works.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
OUT="rust/oracle-tests/golden/settings"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON -DCMAKE_EXPORT_COMPILE_COMMANDS=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_settings
# Outputs only (in/ is a subdirectory and is not matched): every run rewrites the full set,
# so an entry dropped from corpus.txt leaves no stale golden behind.
rm -f "$OUT"/*.cfg "$OUT"/*.toml "$OUT/hashes.txt"
# Run from ROOT: corpus paths are repo-root-relative.
"build/$PRESET/Release/oracle_dump_settings" "$OUT/corpus.txt" "$OUT"
echo "wrote $OUT"
```

Run: `chmod +x /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_settings_golden.sh`
Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_settings_golden.sh`
Expected: FAIL — the build reports no target `oracle_dump_settings` (ninja: `unknown target 'oracle_dump_settings'`). This worktree has no `build/` yet, so the FIRST configure bootstraps vcpkg and is slow; to reuse the main checkout's vcpkg, run the same command as `VCPKG_ROOT=/Users/john/code/openliero/tools/vcpkg/vcpkg bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_settings_golden.sh`.

- [ ] **Step 3: Implement the tool**

Create `src/tools/oracle_dump/settings_dump.cpp`:

```cpp
// Generates the C++ side of the Rust settings-persistence byte gate (Step 4½, slice 4½a-2;
// rust/oracle-tests/tests/settings_toml_golden.rs). For every corpus entry it runs the REAL C++
// persistence path - Settings::load + Settings::save (settings.cpp:62-90, :158-163, i.e.
// FromToml/ToToml), WormSettings::LoadProfile + SaveProfile (worm.cpp:60-95) - and the REAL
// Settings::UpdateHash / WormSettings::UpdateHash (settings.cpp:92-101, worm.cpp:38-43), and
// writes the bytes C++ saves as goldens.
//
// Usage (run from the repo root): oracle_dump_settings <corpus.txt> <out-dir>
// corpus.txt: one entry per line; blank lines and '#' lines are skipped:
//   default-setup <id>        Settings() saved                  -> <id>.cfg, <id>.gameplay.toml
//   default-profile <id>      WormSettings() saved              -> <id>.toml
//   setup <id> <path>         Settings::load(path), then save   -> <id>.cfg, <id>.gameplay.toml
//   profile <id> <path>       LoadProfile(path) over WormSettings(), then SaveProfile -> <id>.toml
// <path> is the rest of the line (it may contain spaces), relative to the repo root.
// <out-dir>/hashes.txt: "xxh3-empty - <XXH3_64 of zero bytes>", then "<kind> <id> <hash>" per
// entry in corpus order (%016llx): Settings::UpdateHash() for setups (whose input is exactly
// <id>.gameplay.toml), WormSettings::UpdateHash() for profiles (whose input is <id>.toml).
//
// Self-checks (each exits 1): every input parses (LoadProfile only warns on a parse error, so
// profiles are pre-parsed with toml::parse); each UpdateHash() equals XXH3_64 of the bytes
// written; and C++ load + save of every file it wrote reproduces that file byte for byte, so
// "a C++-saved file round-trips through Rust" (G5b) is a meaningful check. Built via
// OPENLIERO_BUILD_ORACLE_DUMP (see rust/oracle-tests/gen_settings_golden.sh). Not part of the
// default build.
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <sstream>
#include <string>

#include "filesystem.hpp"
#include "rand.hpp"
#include "settings.hpp"
#include "worm.hpp"

#include <serialization/cereal_types.hpp>
#include <serialization/toml_archive.hpp>
#include <xxhash.h>

namespace {

[[noreturn]] void Fail(std::string const& what) {
  std::fprintf(stderr, "oracle_dump_settings: %s\n", what.c_str());
  std::exit(1);
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

void Spit(std::string const& path, std::string const& bytes) {
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  f.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
  if (!f) {
    Fail("cannot write " + path);
  }
}

uint64_t Xxh3(std::string const& bytes) { return XXH3_64bits(bytes.data(), bytes.size()); }

// The SerializeGameplay TOML exactly as Settings::UpdateHash builds it (settings.cpp:93-98).
std::string GameplayToml(Settings& s) {
  std::ostringstream ss;
  {
    cereal::TomlOutputArchive ar(ss);
    SerializeGameplay(ar, s);
  }
  return ss.str();
}

void EmitHash(std::FILE* hashes, std::string const& kind, std::string const& id, uint64_t hash) {
  std::fprintf(hashes, "%s %s %016" PRIx64 "\n", kind.c_str(), id.c_str(), hash);
}

// Settings::save of `s`, its gameplay bytes and UpdateHash, then the idempotence check.
void EmitSetup(std::FILE* hashes, std::string const& kind, std::string const& id, Settings& s,
               std::string const& dir) {
  Rand rand;
  std::string const kCfgPath = dir + "/" + id + ".cfg";
  s.save(FsNode(kCfgPath), rand);
  std::string const kSaved = Slurp(kCfgPath);
  std::string const kGameplay = GameplayToml(s);
  Spit(dir + "/" + id + ".gameplay.toml", kGameplay);
  uint64_t const kHash = s.UpdateHash();
  if (kHash != Xxh3(kGameplay)) {
    Fail("Settings::UpdateHash() is not XXH3_64 of the gameplay TOML for " + id);
  }
  Settings again;
  if (!again.load(FsNode(kCfgPath), rand) || again.ToToml() != kSaved) {
    Fail("C++ load + save does not reproduce its own output " + kCfgPath);
  }
  EmitHash(hashes, kind, id, kHash);
}

// WormSettings::SaveProfile of `ws` and its UpdateHash, then the idempotence check.
void EmitProfile(std::FILE* hashes, std::string const& kind, std::string const& id,
                 WormSettings& ws, std::string const& dir) {
  std::string const kPath = dir + "/" + id + ".toml";
  ws.SaveProfile(FsNode(kPath));
  std::string const kSaved = Slurp(kPath);
  uint64_t const kHash = ws.UpdateHash();
  if (kHash != Xxh3(kSaved)) {
    Fail("WormSettings::UpdateHash() is not XXH3_64 of the saved profile for " + id);
  }
  WormSettings again;
  again.LoadProfile(FsNode(kPath));
  if (again.ToToml() != kSaved) {
    Fail("C++ LoadProfile + SaveProfile does not reproduce its own output " + kPath);
  }
  EmitHash(hashes, kind, id, kHash);
}

// LoadProfile only logs a parse error (worm.cpp:90-92); the oracle must not fall back silently.
void RequireParses(std::string const& path) {
  try {
    (void)toml::parse(Slurp(path), path);
  } catch (toml::parse_error const& e) {
    Fail(path + ": " + e.what());
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 3) {
    std::fprintf(stderr, "usage: oracle_dump_settings <corpus.txt> <out-dir>\n");
    return 1;
  }
  std::string const kDir = argv[2];
  std::istringstream corpus(Slurp(argv[1]));
  std::FILE* hashes = std::fopen((kDir + "/hashes.txt").c_str(), "w");
  if (!hashes) {
    Fail("cannot write " + kDir + "/hashes.txt");
  }
  std::fprintf(hashes, "xxh3-empty - %016" PRIx64 "\n", Xxh3(std::string()));
  int entries = 0;
  std::string line;
  while (std::getline(corpus, line)) {
    if (line.empty() || line[0] == '#') {
      continue;
    }
    std::istringstream ls(line);
    std::string kind;
    std::string id;
    std::string path;
    ls >> kind >> id;
    std::getline(ls >> std::ws, path);
    if (id.empty()) {
      Fail("bad corpus line: " + line);
    }
    if (kind == "default-setup" && path.empty()) {
      Settings s;
      EmitSetup(hashes, kind, id, s, kDir);
    } else if (kind == "default-profile" && path.empty()) {
      WormSettings ws;
      EmitProfile(hashes, kind, id, ws, kDir);
    } else if (kind == "setup" && !path.empty()) {
      Rand rand;
      Settings s;
      if (!s.load(FsNode(path), rand)) {
        Fail("Settings::load failed on " + path);
      }
      EmitSetup(hashes, kind, id, s, kDir);
    } else if (kind == "profile" && !path.empty()) {
      RequireParses(path);
      WormSettings ws;
      ws.LoadProfile(FsNode(path));
      EmitProfile(hashes, kind, id, ws, kDir);
    } else {
      Fail("bad corpus line: " + line);
    }
    ++entries;
  }
  std::fclose(hashes);
  std::printf("oracle_dump_settings: %d entries\n", entries);
  return 0;
}
```

In `CMakeLists.txt`, directly before the `endif()` that closes `if(OPENLIERO_BUILD_ORACLE_DUMP)` (today after the `oracle_dump_levelgen` pair, `:392-393`; re-read the block first — a sibling slice may have appended its own pair, keep it), add:

```cmake
  add_executable(oracle_dump_settings src/tools/oracle_dump/settings_dump.cpp)
  target_link_libraries(oracle_dump_settings PRIVATE game cereal::cereal)
```

(`game` exports toml++ and xxHash but not cereal; `test_cereal_types` links `cereal::cereal` the same way, `CMakeLists.txt:596-600`.)

- [ ] **Step 4: Generate the goldens**

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_settings_golden.sh`
Expected: the build succeeds, then `oracle_dump_settings: 21 entries` and `wrote rust/oracle-tests/golden/settings`. Any `oracle_dump_settings: …` error line is a self-check failure: an input toml++ rejects, a hash mismatch, or C++ not round-tripping its own output. Diagnose it (the input is the likely culprit — design §9.3.2 lists the inputs C++ cannot round-trip). Never weaken the check.

- [ ] **Step 5: Sanity-check the C++ output against the design**

Run: `ls /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/settings`
Expected: `corpus.txt`, `in`, `hashes.txt`, and the 31 outputs — `defaults.cfg`, `defaults.gameplay.toml`, `default_profile.toml`, `{liero,orbmit,sidecar_killemall,sidecar_scales,sidecar_gametag,quoting,edge,missing_tables,array_player}.{cfg,gameplay.toml}`, `{ai_l,ai_r,joystick0,joystick1,lefty_l,lefty_r,righty_l,righty_r,quoting_profile,edge_profile}.toml`.

Run: `cmp /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/settings/liero.cfg /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/settings/defaults.cfg`
Expected: no output, exit 0 — the shipped legacy `liero.cfg` loads as exactly `Settings()` (design §2.1), so C++ saves identical bytes.

Read `golden/settings/hashes.txt` (Read tool). Expected: 22 lines; line 1 is `xxh3-empty - 2d06800538d394c2` (the `twox-hash` known answer, design §3.4); the `setup liero` and `default-setup defaults` hashes are equal.

Read `golden/settings/default_profile.toml`. Expected exactly (14 lines, one trailing newline, arrays with inner spaces, sorted keys, `rgbDepth = 8`, literal `''` strings):

```
color = 0
controller = 0
controls = [ 0, 0, 0, 0, 0, 0, 0 ]
controlsEx = [ 0, 0, 0, 0, 0, 0, 0, 0 ]
gamepadControls = [ 11, 12, 13, 14, 110, 10, 0, 9 ]
gamepadName = ''
gamepadSerial = ''
health = 100
inputDevice = 0
name = ''
randomName = true
rgb = [ 104, 104, 248 ]
rgbDepth = 8
weapons = [ 1, 1, 1, 1, 1 ]
```

Read `golden/settings/defaults.cfg`. Expected: `[network_player]`, `[player1]`, `[player2]`, `[settings]` in that order with one blank line between tables. It matches the shipped `data/Setups/liero.cfg` except for: `rgb = [ 104, 104, 252 ]` / `[ 60, 172, 60 ]` plus an `rgbDepth = 8` line in each worm table, `maxSpectatorRenderHeight = 1080`, `version = 6`, and a single trailing newline. `weapTable = [` is followed by 40 lines of `    0` (with `,` after all but the last) and a `]` line.

Run (the 4½a-1 generator claims it wrote the sidecars in toml++'s canonical layout):
`cmp /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/sim_slice4_5a_killemall_setup.cfg /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/golden/settings/sidecar_killemall.cfg`
and the same for `scales` and `gametag`.
Expected: identical. If one differs, do NOT touch the sidecar (it is a frozen sim-golden input). Record the difference in the done-report, and tell the T6 implementer to drop the sidecar-identity assertion.

- [ ] **Step 6: clang-format**

Run: `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/settings_dump.cpp`
Expected: no output, exit 0. If it reports anything, run `uvx --from clang-format==22.1.0 clang-format -i --style=file /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/settings_dump.cpp`, re-run the dry run, and re-run Step 4 (a reformat must not change the goldens).

- [ ] **Step 7: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add src/tools/oracle_dump/settings_dump.cpp CMakeLists.txt rust/oracle-tests/gen_settings_golden.sh rust/oracle-tests/golden/settings
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-2): oracle_dump_settings — C++ load+save+UpdateHash goldens for the settings byte gate" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

Reviewer (Opus): the tool calls the real `load`/`save`/`LoadProfile`/`SaveProfile`/`UpdateHash` (no replica beyond `GameplayToml`, which is self-checked against `UpdateHash`); the corpus has 21 entries covering every shipped `data/Setups/*.cfg` and `data/Profiles/*.toml`; every design §3.1 quoting rule has an input; C++ changes are confined to the tool + two CMake lines; clang-format clean.

---

### Task 1: The toml++ formatter port and the settings writer  [Opus]

**Files:**
- Create: `rust/scenario/src/toml_fmt.rs`
- Modify: `rust/scenario/src/lib.rs` (add `mod toml_fmt;` after `pub mod settings_toml;` — private: only `settings_toml` uses it)
- Modify: `rust/scenario/src/settings_toml.rs` — module doc (`:1-10`), imports (`:12-14`), the writer after `load_profile` (`:236`), new tests at the end of `mod tests`

**Interfaces:**
- Consumes: T0's `golden/settings/{defaults.cfg,defaults.gameplay.toml,default_profile.toml}`; 4½a-1's `Settings`, `WormSettings`, `CONFIG_VERSION` (`settings.rs`), `WORM_TABLE_NAMES`, `settings_from_toml`, `load_profile` (`settings_toml.rs`).
- Produces (used by T2, T3, T4, T6):
  - `pub(crate) enum scenario::toml_fmt::Val<'a> { Int(i64), Bool(bool), Str(&'a str), Ints(Vec<i64>) }`, `pub(crate) type KeyVals<'a> = BTreeMap<&'static str, Val<'a>>`, `pub(crate) fn format_doc(values: &KeyVals<'_>, tables: &BTreeMap<&'static str, KeyVals<'_>>) -> String` (no trailing newline), `pub(crate) fn quote_string(s: &str) -> String`.
  - `pub fn scenario::settings_toml::settings_to_toml(s: &Settings) -> String` — `Settings::ToToml` (`settings.cpp:103-131`) = the bytes `Settings::save` writes.
  - `pub fn scenario::settings_toml::worm_settings_to_toml(ws: &WormSettings) -> String` — `WormSettings::ToToml` (`worm.cpp:45-52`) = the bytes `SaveProfile` writes.
  - `pub fn scenario::settings_toml::gameplay_toml(s: &Settings) -> String` — the `SerializeGameplay` bytes `Settings::UpdateHash` hashes (`settings.cpp:92-98`).

Why: design §3.3 / §9.3.3–4. Every Rust TOML serializer disagrees with toml++ on at least one of key order, inner array spaces, the multiline threshold, literal-vs-basic quoting or the trailing newline, so the formatter is transcribed. The port covers what the schema reaches: a root of key/values or of non-inline tables, with integer/bool/string/integer-array values.

- [ ] **Step 1: Write the failing formatter tests** — create `rust/scenario/src/toml_fmt.rs` containing ONLY this test module for now, and add `mod toml_fmt;` to `lib.rs` after `pub mod settings_toml;`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn root(kv: KeyVals<'_>) -> String {
        format_doc(&kv, &BTreeMap::new())
    }

    #[test]
    fn plain_strings_are_literal_and_empty_is_two_single_quotes() {
        assert_eq!(quote_string(""), "''");
        assert_eq!(quote_string("openliero"), "'openliero'");
        assert_eq!(quote_string("Levels/modern_test.lev"), "'Levels/modern_test.lev'");
        assert_eq!(quote_string("AI Joe"), "'AI Joe'");
    }

    #[test]
    fn double_quotes_backslashes_tabs_and_unicode_stay_literal() {
        assert_eq!(quote_string("say \"hi\""), "'say \"hi\"'");
        assert_eq!(quote_string("C:\\pads\\1"), "'C:\\pads\\1'");
        assert_eq!(quote_string("tab\there"), "'tab\there'", "real tabs are allowed");
        assert_eq!(quote_string("Zo\u{e9}"), "'Zo\u{e9}'", "unicode is allowed");
    }

    #[test]
    fn a_single_quote_forces_a_basic_string() {
        assert_eq!(quote_string("O'Brien"), "\"O'Brien\"");
        assert_eq!(quote_string("mix \"q\" 'a' \\x"), "\"mix \\\"q\\\" 'a' \\\\x\"");
        assert_eq!(quote_string("it's\there"), "\"it's\there\"", "a tab stays raw in a basic string");
    }

    #[test]
    fn control_characters_force_a_basic_string_with_the_cpp_escapes() {
        assert_eq!(quote_string("bell\u{1}esc\u{1b}"), "\"bell\\u0001esc\\u001B\"");
        assert_eq!(quote_string("cr\rhere"), "\"cr\\rhere\"");
        assert_eq!(quote_string("\u{8}\u{c}"), "\"\\b\\f\"");
        assert_eq!(quote_string("del\u{7f}"), "\"del\\u007F\"");
        assert_eq!(quote_string("a\u{2028}b"), "\"a\\u2028b\"");
        assert_eq!(
            quote_string("nel\u{85}Zo\u{e9}"),
            "\"nel\\u0085Zo\u{e9}\"",
            "only the vertical space is escaped; the \u{e9} stays raw"
        );
    }

    #[test]
    fn a_newline_makes_a_multi_line_string() {
        assert_eq!(quote_string("two\nlines"), "'''two\nlines'''");
        assert_eq!(quote_string("it's\nmulti"), "'''it's\nmulti'''", "a quote is fine in '''");
        assert_eq!(quote_string("back\\slash\nnext"), "'''back\\slash\nnext'''");
        assert_eq!(
            quote_string("x\ny\u{2}"),
            "\"\"\"x\ny\\u0002\"\"\"",
            "newline + control: multi-line BASIC, the newline raw"
        );
        assert_eq!(quote_string("q\"\n\u{1}"), "\"\"\"q\\\"\n\\u0001\"\"\"");
    }

    #[test]
    fn arrays_wrap_when_the_estimate_reaches_120_columns() {
        // 3 + 38 * (1 + 2) = 117 < 120: inline, with inner spaces.
        let mut kv = KeyVals::new();
        kv.insert("a", Val::Ints(vec![0; 38]));
        let text = root(kv);
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.starts_with("a = [ 0, 0, ") && text.ends_with(", 0 ]"), "{text}");
        // 3 + 39 * 3 = 120 >= 120: one element per line, 4-space indent, `]` on its own line.
        let mut kv = KeyVals::new();
        kv.insert("a", Val::Ints(vec![0; 39]));
        let mut want = String::from("a = [");
        for i in 0..39 {
            if i > 0 {
                want.push(',');
            }
            want.push_str("\n    0");
        }
        want.push_str("\n]");
        assert_eq!(root(kv), want);
    }

    #[test]
    fn integer_widths_count_digits_and_the_sign() {
        assert_eq!(inline_columns(&[0]), 6);
        assert_eq!(inline_columns(&[-1]), 7);
        assert_eq!(inline_columns(&[4_294_967_295; 8]), 99, "the widest schema array stays inline");
        assert_eq!(int_columns(i64::from(i32::MIN)), 11);
        assert_eq!(inline_columns(&[]), 2);
    }

    #[test]
    fn root_values_have_no_leading_or_trailing_newline() {
        let mut kv = KeyVals::new();
        kv.insert("name", Val::Str("x"));
        kv.insert("health", Val::Int(-3));
        kv.insert("randomName", Val::Bool(true));
        kv.insert("rgb", Val::Ints(vec![1, 2, 3]));
        assert_eq!(root(kv), "health = -3\nname = 'x'\nrandomName = true\nrgb = [ 1, 2, 3 ]");
    }

    #[test]
    fn tables_get_headers_and_one_blank_line_between_them() {
        let mut settings = KeyVals::new();
        settings.insert("z", Val::Int(1));
        settings.insert("list", Val::Ints(vec![7; 40]));
        let mut player1 = KeyVals::new();
        player1.insert("k", Val::Bool(false));
        let mut tables = BTreeMap::new();
        tables.insert("settings", settings);
        tables.insert("player1", player1);
        let mut want = String::from("[player1]\nk = false\n\n[settings]\nlist = [");
        for i in 0..40 {
            if i > 0 {
                want.push(',');
            }
            want.push_str("\n    7");
        }
        want.push_str("\n]\nz = 1");
        assert_eq!(format_doc(&KeyVals::new(), &tables), want);
    }

    #[test]
    fn an_empty_array_is_two_brackets() {
        let mut kv = KeyVals::new();
        kv.insert("a", Val::Ints(Vec::new()));
        assert_eq!(root(kv), "a = []");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib toml_fmt::tests`
Expected: FAIL to compile — `cannot find type KeyVals in this scope` / `cannot find function quote_string`.

- [ ] **Step 3: Implement the port** — put this ABOVE the test module in `toml_fmt.rs`:

```rust
//! Step 4½a-2 — a hand port of the subset of toml++ 3.4.0's `toml_formatter` that C++
//! settings persistence reaches (design §3.1, §3.3, §9.3.3). `cereal::TomlOutputArchive`
//! builds a `toml::table` and its destructor streams `out_ << root_ << "\n"`
//! (`toml_archive.hpp:47`), so the bytes C++ saves ARE this formatter's output plus one
//! newline. Sources (vcpkg `tomlplusplus` v3.4.0, `include/toml++/impl/`):
//! `toml_formatter.inl` (table layout, arrays, the 120-column rule), `formatter.inl`
//! (`print_newline`, `print_indent`, `print_string`), `forward_declarations.hpp`
//! (`control_char_escapes`), `unicode.hpp` / `unicode_autogenerated.hpp` (the control and
//! vertical-space classes).
//!
//! Only what the schema reaches is ported: a root holding key/values and/or non-inline
//! sub-tables of key/values, whose values are integers, booleans, strings and integer
//! arrays. `toml::table` is a `std::map<key, …, std::less<>>` (`table.hpp:224`), so keys come
//! out in byte order — a `BTreeMap<&str, _>`. The default `toml_formatter` flags apply
//! (`toml_formatter.hpp:83-91`): literal, multi-line, unicode and real-tab strings allowed,
//! `indentation` on with a 4-space indent. The gate is C++ output, not this reading
//! (`rust/oracle-tests/tests/settings_toml_golden.rs`).

use std::collections::BTreeMap;

/// A value the settings schema writes.
#[derive(Clone, Debug)]
pub(crate) enum Val<'a> {
    Int(i64),
    Bool(bool),
    Str(&'a str),
    Ints(Vec<i64>),
}

/// One table's key/values; the `BTreeMap` gives toml++'s byte-sorted key order.
pub(crate) type KeyVals<'a> = BTreeMap<&'static str, Val<'a>>;

/// `toml_formatter::print(array)`'s wrap limit (`toml_formatter.inl:185`).
const LINE_WRAP_COLS: usize = 120;
/// The `toml_formatter` indent string (`toml_formatter.hpp:100`): 4 columns.
const INDENT: &str = "    ";

/// `control_char_escapes` (`forward_declarations.hpp:127-160`), uppercase hex.
const CONTROL_CHAR_ESCAPES: [&str; 32] = [
    "\\u0000", "\\u0001", "\\u0002", "\\u0003", "\\u0004", "\\u0005", "\\u0006", "\\u0007",
    "\\b", "\\t", "\\n", "\\u000B", "\\f", "\\r", "\\u000E", "\\u000F", "\\u0010", "\\u0011",
    "\\u0012", "\\u0013", "\\u0014", "\\u0015", "\\u0016", "\\u0017", "\\u0018", "\\u0019",
    "\\u001A", "\\u001B", "\\u001C", "\\u001D", "\\u001E", "\\u001F",
];

/// `is_control_character` (`unicode.hpp:97-106`).
fn is_control(c: char) -> bool {
    (c as u32) <= 0x1F || c == '\u{7f}'
}

/// `is_non_ascii_vertical_whitespace` (`unicode_autogenerated.hpp:57-60`).
fn is_non_ascii_vertical_space(c: char) -> bool {
    matches!(c, '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// A key `print(const key&)` emits bare (`print_string(k, false, true, false)`): the schema's
/// keys are all ASCII identifiers.
fn is_bare_key(k: &str) -> bool {
    !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `formatter::print_string` in value context (`formatter.inl:116-358`; `allow_multi_line`,
/// `allow_literal_whitespace` true, `allow_bare` false). Literal (`'…'`) unless the string has
/// a control character, or a `'` without a newline; a newline makes it multi-line (`'''…'''` /
/// `"""…"""`). Basic strings escape `"`, `\`, DEL and C0 controls (tab stays raw, a newline
/// only occurs in a multi-line string and stays raw) and the three non-ASCII vertical spaces;
/// everything else, UTF-8 included, is written raw.
pub(crate) fn quote_string(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut line_breaks = false;
    let mut single_quotes = false;
    let mut control = false;
    for c in s.chars() {
        match c {
            '\n' => line_breaks = true,
            '\t' => {}
            '\'' => single_quotes = true,
            c if is_control(c) || is_non_ascii_vertical_space(c) => control = true,
            _ => {}
        }
    }
    let multi_line = line_breaks;
    let literal = !control && (!single_quotes || multi_line);
    let quot = match (literal, multi_line) {
        (true, false) => "'",
        (true, true) => "'''",
        (false, false) => "\"",
        (false, true) => "\"\"\"",
    };
    let mut out = String::with_capacity(s.len() + 2 * quot.len());
    out.push_str(quot);
    if literal {
        out.push_str(s);
    } else {
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{7f}' => out.push_str("\\u007F"),
                '\t' | '\n' => out.push(c),
                c if (c as u32) <= 0x1F => out.push_str(CONTROL_CHAR_ESCAPES[c as usize]),
                c if is_non_ascii_vertical_space(c) => {
                    out.push_str(&format!("\\u{:04X}", c as u32));
                }
                c => out.push(c),
            }
        }
    }
    out.push_str(quot);
    out
}

/// `toml_formatter_count_inline_columns` for an integer (`toml_formatter.inl:69-81`): 1 for 0,
/// else the digit count plus one for a `-`. C++ takes `log10(double)`, exact for every
/// |v| < 2^53 — all the i32/u32 values the schema stores.
fn int_columns(v: i64) -> usize {
    if v == 0 {
        1
    } else {
        usize::from(v < 0) + v.unsigned_abs().to_string().len()
    }
}

/// `toml_formatter_count_inline_columns` for an integer array (`toml_formatter.inl:46-59`):
/// `3 + Σ(width + 2)`, stopping once it reaches the wrap limit.
fn inline_columns(a: &[i64]) -> usize {
    if a.is_empty() {
        return 2;
    }
    let mut weight = 3;
    for &v in a {
        weight += int_columns(v) + 2;
        if weight >= LINE_WRAP_COLS {
            break;
        }
    }
    weight
}

/// The formatter state (`formatter.hpp:45-49`, `toml_formatter.hpp:51`).
struct Fmt {
    out: String,
    indent: i32,
    naked_newline: bool,
    pending_table_separator: bool,
}

impl Fmt {
    /// `formatter::attach` (`formatter.inl:68-73`): indent 0, nothing printed yet.
    fn new() -> Fmt {
        Fmt {
            out: String::new(),
            indent: 0,
            naked_newline: true,
            pending_table_separator: false,
        }
    }

    /// `print_newline` (`formatter.inl:82-89`): unforced, nothing is printed right after a
    /// newline (or at the very start).
    fn newline(&mut self, force: bool) {
        if !self.naked_newline || force {
            self.out.push('\n');
            self.naked_newline = true;
        }
    }

    /// `print_indent` (`formatter.inl:92-99`): nothing for an indent <= 0.
    fn print_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str(INDENT);
            self.naked_newline = false;
        }
    }

    /// `print_unformatted` (`formatter.inl:102-113`).
    fn raw(&mut self, s: &str) {
        self.out.push_str(s);
        self.naked_newline = false;
    }

    /// `print_pending_table_separator` (`toml_formatter.inl:120-128`): two forced newlines.
    fn pending_separator(&mut self) {
        if self.pending_table_separator {
            self.newline(true);
            self.newline(true);
            self.pending_table_separator = false;
        }
    }

    /// The key/value loop of `print(table)` (`toml_formatter.inl:250-272`).
    fn values(&mut self, kv: &KeyVals<'_>) {
        for (k, v) in kv {
            debug_assert!(is_bare_key(k), "schema keys are bare: {k:?}");
            self.pending_table_separator = true;
            self.newline(false);
            self.print_indent();
            self.raw(k);
            self.raw(" = ");
            match v {
                Val::Int(i) => self.raw(&i.to_string()),
                Val::Bool(b) => self.raw(if *b { "true" } else { "false" }),
                Val::Str(s) => {
                    let quoted = quote_string(s);
                    self.raw(&quoted);
                }
                Val::Ints(a) => self.array(a),
            }
        }
    }

    /// `print(array)` (`toml_formatter.inl:174-235`) for integer elements.
    fn array(&mut self, a: &[i64]) {
        if a.is_empty() {
            self.raw("[]");
            return;
        }
        let original_indent = self.indent;
        let bias = INDENT.len() * usize::try_from(original_indent.max(0)).unwrap_or(0);
        let multiline = inline_columns(a) + bias >= LINE_WRAP_COLS;
        self.raw("[");
        if multiline {
            if original_indent < 0 {
                self.indent = 0;
            }
            self.indent += 1; // indent_array_elements
        } else {
            self.raw(" ");
        }
        for (i, v) in a.iter().enumerate() {
            if i > 0 {
                self.raw(",");
                if !multiline {
                    self.raw(" ");
                }
            }
            if multiline {
                self.newline(true);
                self.print_indent();
            }
            self.raw(&v.to_string());
        }
        if multiline {
            self.indent = original_indent;
            self.newline(true);
            self.print_indent();
        } else {
            self.raw(" ");
        }
        self.raw("]");
    }
}

/// `toml_formatter::print()` on a root table (`toml_formatter.inl:376-393`, then
/// `print(table)` `:238-373`): the root's key/values, then each sub-table under a `[name]`
/// header, one blank line between tables. No trailing newline — `TomlOutputArchive`'s
/// destructor appends the only `"\n"`.
pub(crate) fn format_doc(
    values: &KeyVals<'_>,
    tables: &BTreeMap<&'static str, KeyVals<'_>>,
) -> String {
    let mut f = Fmt::new();
    f.indent -= 1; // `decrease_indent()`: root key/values and tables share one indent
    f.values(values);
    for (name, kv) in tables {
        debug_assert!(is_bare_key(name), "table names are bare: {name:?}");
        f.pending_separator();
        f.indent += 1; // indent_sub_tables
        f.print_indent();
        f.raw("[");
        f.raw(name);
        f.raw("]");
        f.pending_table_separator = true;
        f.values(kv);
        f.indent -= 1;
    }
    f.out
}
```

- [ ] **Step 4: Run the formatter tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib toml_fmt::tests`
Expected: PASS (10 tests). (Dead-code warnings for `format_doc`/`Val` outside tests are expected until Step 6.)

- [ ] **Step 5: Write the failing writer tests** — append inside `mod tests` in `settings_toml.rs` (after `profile_keys_are_read_at_the_root_and_the_colour_is_preserved`):

```rust
    const GOLDEN_SETTINGS: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../oracle-tests/golden/settings");

    /// A C++-generated golden (T0, `gen_settings_golden.sh`).
    fn golden(name: &str) -> String {
        std::fs::read_to_string(format!("{GOLDEN_SETTINGS}/{name}"))
            .unwrap_or_else(|e| panic!("read golden {name}: {e}"))
    }

    #[test]
    fn the_default_profile_is_the_cpp_worm_settings_to_toml() {
        // Sorted keys, inner-spaced inline arrays, literal '' strings, rgbDepth = 8 on save,
        // exactly one trailing newline (the archive's "\n").
        let want = "color = 0\ncontroller = 0\ncontrols = [ 0, 0, 0, 0, 0, 0, 0 ]\n\
                    controlsEx = [ 0, 0, 0, 0, 0, 0, 0, 0 ]\n\
                    gamepadControls = [ 11, 12, 13, 14, 110, 10, 0, 9 ]\n\
                    gamepadName = ''\ngamepadSerial = ''\nhealth = 100\ninputDevice = 0\n\
                    name = ''\nrandomName = true\nrgb = [ 104, 104, 248 ]\nrgbDepth = 8\n\
                    weapons = [ 1, 1, 1, 1, 1 ]\n";
        assert_eq!(worm_settings_to_toml(&WormSettings::default()), want);
    }

    #[test]
    fn the_writer_reproduces_the_cpp_defaults_goldens() {
        assert_eq!(settings_to_toml(&Settings::default()), golden("defaults.cfg"));
        assert_eq!(gameplay_toml(&Settings::default()), golden("defaults.gameplay.toml"));
        assert_eq!(
            worm_settings_to_toml(&WormSettings::default()),
            golden("default_profile.toml")
        );
    }

    #[test]
    fn the_settings_table_has_37_keys_and_the_gameplay_subset_28() {
        let full = settings_to_toml(&Settings::default());
        assert!(full.starts_with("[network_player]\n"), "tables are byte-sorted");
        assert!(full.ends_with("zoneTimeout = 30\n") && !full.ends_with("\n\n"));
        let settings_table = full.split("[settings]\n").nth(1).expect("a [settings] table");
        assert_eq!(settings_table.lines().filter(|l| l.contains(" = ")).count(), 37);
        let gameplay = gameplay_toml(&Settings::default());
        assert_eq!(gameplay.lines().filter(|l| l.contains(" = ")).count(), 28);
        for absent in [
            "version",
            "modernColors",
            "fullscreen",
            "singleScreenReplay",
            "spectatorWindow",
            "bloodParticleMax",
            "randomMapWidth",
            "randomMapHeight",
            "maxSpectatorRenderHeight",
            "rgbDepth",
        ] {
            assert!(
                !gameplay.contains(&format!("{absent} =")),
                "{absent} is not in SerializeGameplay"
            );
        }
        assert!(!gameplay.starts_with('['), "the gameplay subset sits at the root");
        assert!(gameplay.starts_with("aiFrames = 140\n"));
    }

    #[test]
    fn writing_then_reading_is_the_identity() {
        let mut s = Settings::default();
        s.tc = "it's\ta \"tc\"".to_string();
        s.level_file = "two\nlines".to_string();
        s.lives = -4;
        s.game_mode = u32::MAX;
        s.weap_table[7] = 2;
        s.map = false;
        s.worm_settings[0].name = "Zo\u{e9}\u{2028}".to_string();
        s.worm_settings[1].rgb = [0, 255, 7];
        s.worm_settings[2].controls_ex = [u32::MAX; 8];
        assert_eq!(settings_from_toml(&settings_to_toml(&s)).unwrap(), s);
        let ws = s.worm_settings[0].clone();
        let mut back = WormSettings::default();
        back.color = ws.color; // LoadProfile restores the pre-load colour
        load_profile(&worm_settings_to_toml(&ws), &mut back).unwrap();
        assert_eq!(back, ws);
    }
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests`
Expected: FAIL to compile — `cannot find function worm_settings_to_toml in this scope` (and `settings_to_toml`, `gameplay_toml`).

- [ ] **Step 6: Implement the writer** in `settings_toml.rs`.

Replace the last sentence of the module doc (`:10`, "… expansion (`cereal_types.hpp:288-303`). The writer is 4½a-2.") with:

```rust
//! expansion (`cereal_types.hpp:288-303`).
//!
//! Step 4½a-2 adds the **writer** (`Settings::ToToml`, `WormSettings::ToToml`, the
//! `SerializeGameplay` subset — design §3.3): the keys `TomlOutputArchive` inserts, laid out by
//! the toml++ formatter port in `crate::toml_fmt`, plus the archive's closing `"\n"`.
```

Replace the imports (`:12-14`) with:

```rust
use std::collections::BTreeMap;

use toml::{Table, Value};

use crate::settings::{Settings, WormSettings, CONFIG_VERSION};
use crate::toml_fmt::{format_doc, KeyVals, Val};
```

Insert after `load_profile` (before `#[cfg(test)]`):

```rust
// --- Writer (Step 4½a-2, design §3.3) ----------------------------------------------------
// The key ORDER below follows cereal_types.hpp for review only: `toml::table` sorts keys, so
// the BTreeMap decides the byte order, exactly as in C++.

/// An integer array as `TomlOutputArchive` stores it: every element widened to `int64`
/// (`toml_archive.hpp:71-74`; `uint32_t` values are therefore never negative).
fn ints<'a, T: Copy + Into<i64>>(a: &[T]) -> Val<'a> {
    Val::Ints(a.iter().map(|&v| v.into()).collect())
}

/// `SerializeWormSettingsToml` on save (`cereal_types.hpp:282-308`): `rgbDepth` is 8.
fn worm_keys(ws: &WormSettings) -> KeyVals<'_> {
    let mut kv = KeyVals::new();
    kv.insert("name", Val::Str(&ws.name));
    kv.insert("health", Val::Int(ws.health.into()));
    kv.insert("controller", Val::Int(ws.controller.into()));
    kv.insert("randomName", Val::Bool(ws.random_name));
    kv.insert("color", Val::Int(ws.color.into()));
    kv.insert("inputDevice", Val::Int(ws.input_device.into()));
    kv.insert("gamepadName", Val::Str(&ws.gamepad_name));
    kv.insert("gamepadSerial", Val::Str(&ws.gamepad_serial));
    kv.insert("rgbDepth", Val::Int(8));
    kv.insert("rgb", ints(&ws.rgb));
    kv.insert("weapons", ints(&ws.weapons));
    kv.insert("controls", ints(&ws.controls));
    kv.insert("controlsEx", ints(&ws.controls_ex));
    kv.insert("gamepadControls", ints(&ws.gamepad_controls));
    kv
}

/// `SerializeGameplay` (`cereal_types.hpp:218-239`) — the `UpdateHash` subset: the
/// `GameplayExtensions`, the gameplay scalars, `weapTable`, `bonusTimeout`, `inputDelay`.
/// `ToToml`'s `[settings]` is this plus nine more keys (`settings_to_toml`).
fn gameplay_keys(s: &Settings) -> KeyVals<'_> {
    let mut kv = KeyVals::new();
    kv.insert("recordReplays", Val::Bool(s.record_replays));
    kv.insert("loadPowerlevelPalette", Val::Bool(s.load_powerlevel_palette));
    kv.insert("aiFrames", Val::Int(s.ai_frames.into()));
    kv.insert("aiMutations", Val::Int(s.ai_mutations.into()));
    kv.insert("aiTraces", Val::Bool(s.ai_traces));
    kv.insert("aiParallels", Val::Int(s.ai_parallels.into()));
    kv.insert("zoneTimeout", Val::Int(s.zone_timeout.into()));
    kv.insert("selectBotWeapons", Val::Int(s.select_bot_weapons.into()));
    kv.insert("allowViewingSpawnPoint", Val::Bool(s.allow_viewing_spawn_point));
    kv.insert("tc", Val::Str(&s.tc));
    kv.insert("maxBonuses", Val::Int(s.max_bonuses.into()));
    kv.insert("blood", Val::Int(s.blood.into()));
    kv.insert("timeToLose", Val::Int(s.time_to_lose.into()));
    kv.insert("flagsToWin", Val::Int(s.flags_to_win.into()));
    kv.insert("gameMode", Val::Int(s.game_mode.into()));
    kv.insert("shadow", Val::Bool(s.shadow));
    kv.insert("loadChange", Val::Bool(s.load_change));
    kv.insert("namesOnBonuses", Val::Bool(s.names_on_bonuses));
    kv.insert("regenerateLevel", Val::Bool(s.regenerate_level));
    kv.insert("lives", Val::Int(s.lives.into()));
    kv.insert("loadingTime", Val::Int(s.loading_time.into()));
    kv.insert("randomLevel", Val::Bool(s.random_level));
    kv.insert("levelFile", Val::Str(&s.level_file));
    kv.insert("map", Val::Bool(s.map));
    kv.insert("screenSync", Val::Bool(s.screen_sync));
    kv.insert("weapTable", ints(&s.weap_table));
    kv.insert("bonusTimeout", Val::Int(s.bonus_timeout.into()));
    kv.insert("inputDelay", Val::Int(s.input_delay.into()));
    kv
}

/// `~TomlOutputArchive` (`toml_archive.hpp:47`): `out_ << root_ << "\n"`.
fn archive_bytes(values: &KeyVals<'_>, tables: &BTreeMap<&'static str, KeyVals<'_>>) -> String {
    let mut out = format_doc(values, tables);
    out.push('\n');
    out
}

/// `Settings::ToToml` (`settings.cpp:103-131`) — the bytes `Settings::save` writes: a
/// `[settings]` table (`version` = `kConfigVersion`, `modernColors`,
/// `SerializeSettingsScalars`, `weapTable`) and the three worm tables.
pub fn settings_to_toml(s: &Settings) -> String {
    let mut st = gameplay_keys(s);
    st.insert("version", Val::Int(CONFIG_VERSION.into()));
    st.insert("modernColors", Val::Bool(s.modern_colors));
    st.insert("fullscreen", Val::Bool(s.fullscreen));
    st.insert("singleScreenReplay", Val::Bool(s.single_screen_replay));
    st.insert("spectatorWindow", Val::Bool(s.spectator_window));
    st.insert("bloodParticleMax", Val::Int(s.blood_particle_max.into()));
    st.insert("randomMapWidth", Val::Int(s.random_map_width.into()));
    st.insert("randomMapHeight", Val::Int(s.random_map_height.into()));
    st.insert(
        "maxSpectatorRenderHeight",
        Val::Int(s.max_spectator_render_height.into()),
    );
    let mut tables = BTreeMap::new();
    tables.insert("settings", st);
    for (name, ws) in WORM_TABLE_NAMES.iter().zip(&s.worm_settings) {
        tables.insert(*name, worm_keys(ws));
    }
    archive_bytes(&KeyVals::new(), &tables)
}

/// `WormSettings::ToToml` (`worm.cpp:45-52`) — the bytes `SaveProfile` writes (`:60-71`):
/// the profile keys at the root.
pub fn worm_settings_to_toml(ws: &WormSettings) -> String {
    archive_bytes(&worm_keys(ws), &BTreeMap::new())
}

/// The `SerializeGameplay` TOML `Settings::UpdateHash` hashes (`settings.cpp:92-98`): the
/// gameplay keys at the root.
pub fn gameplay_toml(s: &Settings) -> String {
    archive_bytes(&gameplay_keys(s), &BTreeMap::new())
}
```

- [ ] **Step 7: Run the tests and the format checks**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests`
Expected: PASS (16 tests). If `the_writer_reproduces_the_cpp_defaults_goldens` fails while `the_default_profile_is_the_cpp_worm_settings_to_toml` passes (or vice versa), the C++ bytes differ from design §3.1's reading. The golden is the truth: fix `toml_fmt`/the writer (and, if §3.1 was wrong, note it for the T7 design update).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario` — Expected: PASS (everything, no warnings from `toml_fmt`).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/toml_fmt.rs` — Expected: no output (else run it without `--check` and re-test).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/settings_toml.rs` — a read-only guide (existing file): apply by hand only hunks inside the code this task added; expected none or only those.

- [ ] **Step 8: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/toml_fmt.rs rust/scenario/src/settings_toml.rs rust/scenario/src/lib.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-2): toml++ formatter port + settings/profile/gameplay TOML writer" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

Reviewer (Opus): every branch of `quote_string`/`array`/`format_doc` cites and matches the toml++ source line it transcribes (design §9.3.3); `gameplay_keys` is exactly `SerializeGameplay`'s 28 keys and `settings_to_toml` adds exactly the other nine `ToToml` keys; no serde, no new dependency.

---

### Task 2: The byte gate — G5a (load + save parity) and G5b (C++-saved round trip)  [Sonnet]

**Files:**
- Create: `rust/oracle-tests/tests/settings_toml_golden.rs`

**Interfaces:**
- Consumes: T0's `golden/settings/corpus.txt` and outputs; T1's `settings_to_toml`, `worm_settings_to_toml`; 4½a-1's `settings_from_toml`, `load_profile`, `Settings`, `WormSettings`.
- Produces (extended by T3 and T6): the test file's helpers — `GOLDEN`, `REPO`, `read(&Path) -> String`, `golden(&str) -> String`, `struct Entry { kind, id, path }` with `is_setup()`, `input()`, `load_setup() -> Settings`, `load_profile() -> WormSettings`, `saved_name() -> String`, `corpus() -> Vec<Entry>`, `entry(&str) -> Entry`, `assert_same(label, got, want)`.

Why: design §3.4 G5a + G5b, Hard gate 5. The test replays every corpus input through the real Rust reader and writer and compares with what C++ wrote, then feeds the C++ output back through Rust. There is no separate RED here: T1 was the RED (the writer did not exist); this gate either passes or exposes a reader/writer divergence on a real input.

- [ ] **Step 1: Write the gate** — create `rust/oracle-tests/tests/settings_toml_golden.rs`:

```rust
//! Step 4½a-2 — Hard gate 5, the settings TOML byte gate (design §3.4), against the C++ oracle
//! `oracle_dump_settings` (`src/tools/oracle_dump/settings_dump.cpp`, regenerated by
//! `gen_settings_golden.sh`). `golden/settings/corpus.txt` lists every input; the C++ tool and
//! this test both read it.
//!
//! - **G5a**: `rust_save(rust_load(I)) == cpp_save(cpp_load(I))` for every corpus input `I`.
//! - **G5b**: every C++-saved file `O` round-trips — `rust_save(rust_load(O)) == O` — and
//!   `rust_load(O) == rust_load(I)`.
//! - **G5c** (T3): the gameplay bytes and both `UpdateHash` vectors.
//!
//! Setups load like `Gfx::LoadSettings` (a fresh `Settings`, then `FromToml`); profiles like
//! `LoadProfile` over a bare `WormSettings()` — the tool's starting values (design §9.3.1).

use std::path::Path;

use scenario::settings::{Settings, WormSettings};
use scenario::settings_toml::{
    load_profile, settings_from_toml, settings_to_toml, worm_settings_to_toml,
};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/settings");
const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A file under `golden/settings/`.
fn golden(name: &str) -> String {
    read(&Path::new(GOLDEN).join(name))
}

/// One `corpus.txt` entry: `<kind> <id> [<path>]`, `<path>` = the rest of the line.
struct Entry {
    kind: String,
    id: String,
    path: Option<String>,
}

impl Entry {
    fn is_setup(&self) -> bool {
        self.kind == "setup" || self.kind == "default-setup"
    }

    /// The input file's text (repo-root-relative path).
    fn input(&self) -> String {
        read(&Path::new(REPO).join(self.path.as_deref().expect("an input path")))
    }

    /// `Gfx::LoadSettings`: a fresh `Settings`, then `FromToml` — or `Settings()` itself.
    fn load_setup(&self) -> Settings {
        match self.kind.as_str() {
            "default-setup" => Settings::default(),
            "setup" => settings_from_toml(&self.input())
                .unwrap_or_else(|e| panic!("{}: {e}", self.id)),
            k => panic!("{}: not a setup entry ({k})", self.id),
        }
    }

    /// `LoadProfile` over a bare `WormSettings()` — or `WormSettings()` itself.
    fn load_profile(&self) -> WormSettings {
        let mut ws = WormSettings::default();
        match self.kind.as_str() {
            "default-profile" => {}
            "profile" => load_profile(&self.input(), &mut ws)
                .unwrap_or_else(|e| panic!("{}: {e}", self.id)),
            k => panic!("{}: not a profile entry ({k})", self.id),
        }
        ws
    }

    /// The file C++ saved for this entry.
    fn saved_name(&self) -> String {
        if self.is_setup() {
            format!("{}.cfg", self.id)
        } else {
            format!("{}.toml", self.id)
        }
    }
}

fn corpus() -> Vec<Entry> {
    golden("corpus.txt")
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut it = l.splitn(3, ' ');
            Entry {
                kind: it.next().expect("kind").to_string(),
                id: it.next().expect("id").to_string(),
                path: it.next().map(str::to_string),
            }
        })
        .collect()
}

fn entry(id: &str) -> Entry {
    corpus()
        .into_iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("no corpus entry {id}"))
}

/// Byte equality, reporting the first differing line.
fn assert_same(label: &str, got: &str, want: &str) {
    if got == want {
        return;
    }
    let g: Vec<&str> = got.split('\n').collect();
    let w: Vec<&str> = want.split('\n').collect();
    let i = g
        .iter()
        .zip(&w)
        .position(|(a, b)| a != b)
        .unwrap_or(g.len().min(w.len()));
    panic!(
        "{label}: rust {} bytes vs C++ {} bytes; first difference at line {}:\n  rust: {:?}\n  c++:  {:?}",
        got.len(),
        want.len(),
        i + 1,
        g.get(i),
        w.get(i)
    );
}

#[test]
fn g5a_rust_load_and_save_equals_cpp_load_and_save() {
    let entries = corpus();
    assert_eq!(entries.len(), 21, "corpus.txt entries");
    for e in &entries {
        let got = if e.is_setup() {
            settings_to_toml(&e.load_setup())
        } else {
            worm_settings_to_toml(&e.load_profile())
        };
        assert_same(&format!("G5a {} ({})", e.id, e.kind), &got, &golden(&e.saved_name()));
    }
}

#[test]
fn g5b_cpp_saved_files_round_trip_through_rust() {
    for e in &corpus() {
        let saved = golden(&e.saved_name());
        if e.is_setup() {
            let back = settings_from_toml(&saved).unwrap_or_else(|err| panic!("{}: {err}", e.id));
            assert_same(&format!("G5b {}", e.id), &settings_to_toml(&back), &saved);
            assert_eq!(back, e.load_setup(), "G5b {}: rust_load(O) == rust_load(I)", e.id);
        } else {
            let mut back = WormSettings::default();
            load_profile(&saved, &mut back).unwrap_or_else(|err| panic!("{}: {err}", e.id));
            assert_same(&format!("G5b {}", e.id), &worm_settings_to_toml(&back), &saved);
            assert_eq!(back, e.load_profile(), "G5b {}: rust_load(O) == rust_load(I)", e.id);
        }
    }
}
```

(`entry` is used from T6 on; until then the compiler warns that it is unused — expected.)

- [ ] **Step 2: Run the gate**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test settings_toml_golden`
Expected: PASS (2 tests).

If G5a fails for an entry, use G5b to localise it. If G5b passes for that entry's output, the writer is right and the **reader** disagrees with C++ on that input. The likeliest place is `array_player` (design §3.2's positional reading, confirmed here against C++ for the first time) — fix `settings_toml.rs`'s `Frame` to match C++ and record the fact for T7's design update. If G5b also fails, the **writer** is wrong: fix `toml_fmt`. Never edit a golden by hand.

- [ ] **Step 3: Format and commit**

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/settings_toml_golden.rs` — Expected: no output (else run without `--check` and re-test).

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/settings_toml_golden.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-2): settings byte gate G5a/G5b — Rust load+save == C++ load+save on the corpus" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

(If Step 2 needed a reader or writer fix, add those files to the commit and say so in the message body.)

---

### Task 3: `UpdateHash` — `twox-hash`, `update_hash`, `worm_update_hash`, G5c  [Sonnet]

**Files:**
- Modify: `rust/scenario/Cargo.toml` (`[dependencies]`), `rust/Cargo.lock` (cargo updates it)
- Modify: `rust/scenario/src/settings_toml.rs` (import; two functions after `gameplay_toml`; three tests)
- Modify: `rust/oracle-tests/tests/settings_toml_golden.rs` (imports; `hashes()`; one test)

**Interfaces:**
- Consumes: T1's `gameplay_toml`, `worm_settings_to_toml`; T0's `hashes.txt` and `<id>.gameplay.toml`.
- Produces (used by T6; Step 5's hash exchange later):
  - `pub fn scenario::settings_toml::update_hash(s: &Settings) -> u64` — `Settings::UpdateHash()`.
  - `pub fn scenario::settings_toml::worm_update_hash(ws: &WormSettings) -> u64` — `WormSettings::UpdateHash()`.
  - In the golden test: `fn hashes() -> BTreeMap<String, u64>` (id ↦ hash; the KAT line under `"-"`).

Why: design §3.4 G5c, LD 8. `UpdateHash` = XXH3-64, seed 0, over the bytes T1 writes: `SerializeGameplay` for `Settings` (`settings.cpp:92-101`), the profile TOML for `WormSettings` (`worm.cpp:38-43`). The crate choice and its exact pin are design §3.4 / §9.3.5.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `settings_toml.rs`:

```rust
    use twox_hash::XxHash3_64;

    #[test]
    fn xxh3_64_known_answer() {
        // The crate's own vector (`xxhash3_64.rs`, `oneshot_empty`); the C++ oracle writes the
        // same value as `hashes.txt`'s first line.
        assert_eq!(XxHash3_64::oneshot(b""), 0x2d06_8005_38d3_94c2);
    }

    #[test]
    fn update_hash_is_xxh3_of_the_bytes_cpp_hashes() {
        let s = Settings::default();
        assert_eq!(update_hash(&s), XxHash3_64::oneshot(gameplay_toml(&s).as_bytes()));
        let ws = WormSettings::default();
        assert_eq!(
            worm_update_hash(&ws),
            XxHash3_64::oneshot(worm_settings_to_toml(&ws).as_bytes())
        );
    }

    #[test]
    fn only_gameplay_fields_move_update_hash() {
        let base = update_hash(&Settings::default());
        let mut s = Settings::default();
        s.fullscreen = true;
        s.modern_colors = true;
        s.blood_particle_max = 5;
        s.random_map_width = 640;
        s.max_spectator_render_height = 720;
        s.worm_settings[0].name = "x".to_string();
        assert_eq!(
            update_hash(&s),
            base,
            "AppSettings, the map size and the worms are outside SerializeGameplay (design §9.3.9)"
        );
        s.lives = 3;
        assert_ne!(update_hash(&s), base);
        let mut t = Settings::default();
        t.weap_table[39] = 1;
        assert_ne!(update_hash(&t), base, "weapTable is hashed");
    }
```

Add the dependency to `rust/scenario/Cargo.toml`, directly under `toml = "0.8"`:

```toml
# Step 4½a-2: XXH3-64 for Settings/WormSettings::UpdateHash (design §3.4, §9.3.5). Exact pin:
# the vetted source, resolvable offline from the local cargo cache. Pure Rust, no
# dependencies with default features off; wasm32 takes its scalar path.
twox-hash = { version = "=2.1.3", default-features = false, features = ["xxhash3_64"] }
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests`
Expected: FAIL to compile — `cannot find function update_hash in this scope` (and `worm_update_hash`). If cargo instead fails to fetch the index (no network), re-run the same command with `--offline` appended: the `.crate` and its index entry are in the local cache.

- [ ] **Step 2: Implement** — in `settings_toml.rs` add `use twox_hash::XxHash3_64;` after `use toml::{Table, Value};`, and after `gameplay_toml`:

```rust
/// `Settings::UpdateHash` (`settings.cpp:92-101`): XXH3-64 (seed 0) over [`gameplay_toml`].
pub fn update_hash(s: &Settings) -> u64 {
    XxHash3_64::oneshot(gameplay_toml(s).as_bytes())
}

/// `WormSettings::UpdateHash` (`worm.cpp:38-43`): XXH3-64 (seed 0) over
/// [`worm_settings_to_toml`] — exactly the bytes `SaveProfile` writes.
pub fn worm_update_hash(ws: &WormSettings) -> u64 {
    XxHash3_64::oneshot(worm_settings_to_toml(ws).as_bytes())
}
```

(With the module-level import in place, drop the `use twox_hash::XxHash3_64;` line you added inside `mod tests` if the compiler reports it as redundant.)

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib settings_toml::tests` — Expected: PASS (19 tests).
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario -i twox-hash` — Expected: `twox-hash v2.1.3` with `scenario v0.1.0` as its only dependent path root (`scenario` → `oracle-tests`/`shot`/`game`).
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p twox-hash --depth 1` — Expected: the single line `twox-hash v2.1.3` (no dependencies).

- [ ] **Step 3: Add G5c to the gate** — in `settings_toml_golden.rs` add `use std::collections::BTreeMap;` above `use std::path::Path;`, extend the `settings_toml` import to `{gameplay_toml, load_profile, settings_from_toml, settings_to_toml, update_hash, worm_settings_to_toml, worm_update_hash}`, and append:

```rust
/// `hashes.txt`: `<kind> <id> <hash %016x>` → id ↦ hash; the `xxh3-empty -` KAT sits under `"-"`.
fn hashes() -> BTreeMap<String, u64> {
    golden("hashes.txt")
        .lines()
        .map(|l| {
            let cols: Vec<&str> = l.split(' ').collect();
            assert_eq!(cols.len(), 3, "hashes.txt line {l:?}");
            (cols[1].to_string(), u64::from_str_radix(cols[2], 16).expect("hex hash"))
        })
        .collect()
}

#[test]
fn g5c_gameplay_bytes_and_update_hash_match_cpp() {
    let hashes = hashes();
    assert_eq!(
        hashes["-"],
        0x2d06_8005_38d3_94c2,
        "C++ XXH3_64 of zero bytes == the twox-hash known answer"
    );
    let entries = corpus();
    assert_eq!(hashes.len(), entries.len() + 1, "one hash per corpus entry + the KAT");
    for e in &entries {
        let want = hashes[&e.id];
        if e.is_setup() {
            let s = e.load_setup();
            assert_same(
                &format!("G5c {} gameplay", e.id),
                &gameplay_toml(&s),
                &golden(&format!("{}.gameplay.toml", e.id)),
            );
            assert_eq!(update_hash(&s), want, "G5c {}: Settings::UpdateHash", e.id);
        } else {
            let ws = e.load_profile();
            assert_eq!(worm_update_hash(&ws), want, "G5c {}: WormSettings::UpdateHash", e.id);
        }
    }
}
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test settings_toml_golden` — Expected: PASS (3 tests). A gameplay-bytes mismatch with G5a green means `gameplay_keys` has a key too many or too few; a hash mismatch with identical bytes would mean a different XXH3 variant (seed/secret) — the KAT assertion rules that out.
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/settings_toml_golden.rs` — Expected: no output (this plan created the file; format it if needed).

- [ ] **Step 4: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/Cargo.toml rust/Cargo.lock rust/scenario/src/settings_toml.rs rust/oracle-tests/tests/settings_toml_golden.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-2): Settings/WormSettings UpdateHash (XXH3-64 via twox-hash =2.1.3) + G5c vs C++ vectors" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

---

### Task 4: `scenario::paths` + `scenario::storage` (the `ConfigStore` seam) + `TC_ROOT` centralised  [Sonnet]

**Files:**
- Create: `rust/scenario/src/paths.rs`, `rust/scenario/src/storage.rs`
- Modify: `rust/scenario/src/lib.rs` (add `pub mod paths;` after `pub mod parser;` and `pub mod storage;` after `pub mod settings_toml;`)
- Modify: `rust/game/src/main.rs:35-37` (the `TC_ROOT` const), `rust/shot/src/lib.rs:392-397` (the default TC root)

**Interfaces:**
- Consumes: T1's `settings_to_toml`; 4½a-1's `settings_from_toml`, `Settings`.
- Produces (used by T5, 4½d, 4½e, 4½h):
  - `pub const scenario::paths::DATA_ROOT: &str` (the repo's `data/`), `pub const scenario::paths::TC_ROOT: &str` (`data/TC/openliero`).
  - `pub trait scenario::storage::ConfigStore { fn read(&self, rel: &str) -> Option<Vec<u8>>; fn write(&self, rel: &str, bytes: &[u8]) -> std::io::Result<()>; fn shadows_system(&self, subdir: &str, leaf: &str) -> bool; }`
  - `pub struct NativeStore` — `split(user_root: PathBuf, system_root: Option<PathBuf>) -> NativeStore`, `single_dir(root: PathBuf) -> NativeStore`, `resolve_default() -> Option<NativeStore>`, `resolve_with(os: PrefOs, env: &dyn Fn(&str) -> Option<String>) -> Option<NativeStore>`, `user_root(&self) -> &Path`, `system_root(&self) -> Option<&Path>`.
  - `pub struct MemoryStore` (`Default`) — `new() -> MemoryStore`, `with_system(files: &[(&str, &[u8])]) -> MemoryStore`, `user_file(&self, rel: &str) -> Option<Vec<u8>>`.
  - `pub enum PrefOs { MacOs, Unix, Windows, Other }` + `PrefOs::current()`; `pub fn pref_path_for(os: PrefOs, env: &dyn Fn(&str) -> Option<String>, org: &str, app: &str) -> Option<String>`; `pub fn pref_path(org: &str, app: &str) -> Option<PathBuf>`.
  - `pub fn is_reserved(subdir: &str, leaf: &str) -> bool`; `pub fn load_setup(store: &dyn ConfigStore) -> std::io::Result<Settings>`; `pub fn save_setup(store: &dyn ConfigStore, s: &Settings) -> std::io::Result<()>`; consts `SETUP_REL`, `TEST_USER_DIR_ENV`, `PREF_ORG`, `PREF_APP`.

Why: design §3.5, §2.2, §9.3.7–8. Native storage mirrors `paths::Resolve` (`filesystem.cpp:767-845`): merged reads (user dir, then the read-only `data/`), writes to the user dir only, and `ShadowsSystem` (`:736-763`) for 4½e's Save As dialogs. The user dir is SDL3's `SDL_GetPrefPath("openliero", "openliero")`, recomputed from environment variables with no `dirs` crate and no new dependency. `MemoryStore` covers the unit tests and is the wasm stand-in until 4½h. `load_setup`/`save_setup` are `gameEntry.cpp:55-58`/`:78`. `TC_ROOT` is centralised for its two production users; the per-test consts stay (§2.2).

- [ ] **Step 1: Write the failing tests** — create `rust/scenario/src/paths.rs` and `rust/scenario/src/storage.rs` containing ONLY their test modules, and add both `pub mod` lines to `lib.rs`.

`paths.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_roots_point_at_the_repo_data() {
        assert!(Path::new(DATA_ROOT).join("Setups/liero.cfg").is_file());
        assert!(Path::new(TC_ROOT).join("tc.cfg").is_file());
        assert_eq!(TC_ROOT.strip_prefix(DATA_ROOT), Some("/TC/openliero"));
    }
}
```

`storage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::DATA_ROOT;

    /// A fresh per-process scratch directory (the `shot` tests' `temp_dir` + pid pattern).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "liero_rs_storage_{}_{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, bytes).expect("write");
    }

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    fn shipped(rel: &str) -> Vec<u8> {
        std::fs::read(Path::new(DATA_ROOT).join(rel)).expect("shipped file")
    }

    #[test]
    fn pref_path_mirrors_sdl_get_pref_path_per_platform() {
        let home = env(&[("HOME", "/Users/j")]);
        assert_eq!(
            pref_path_for(PrefOs::MacOs, &home, "openliero", "openliero").as_deref(),
            Some("/Users/j/Library/Application Support/openliero/openliero/")
        );
        assert_eq!(
            pref_path_for(PrefOs::Unix, &home, "openliero", "openliero").as_deref(),
            Some("/Users/j/.local/share/openliero/openliero/")
        );
        let xdg = env(&[("HOME", "/home/j"), ("XDG_DATA_HOME", "/data/")]);
        assert_eq!(
            pref_path_for(PrefOs::Unix, &xdg, "o", "a").as_deref(),
            Some("/data/o/a/")
        );
        let win = env(&[("APPDATA", "C:\\Users\\j\\AppData\\Roaming")]);
        assert_eq!(
            pref_path_for(PrefOs::Windows, &win, "openliero", "openliero").as_deref(),
            Some("C:\\Users\\j\\AppData\\Roaming\\openliero\\openliero\\")
        );
        assert_eq!(pref_path_for(PrefOs::Other, &home, "o", "a"), None);
    }

    #[test]
    fn pref_path_treats_missing_and_empty_variables_as_unset() {
        let none = env(&[]);
        for os in [PrefOs::MacOs, PrefOs::Unix, PrefOs::Windows] {
            assert_eq!(pref_path_for(os, &none, "o", "a"), None, "{os:?}");
        }
        let empty = env(&[("HOME", "/home/j"), ("XDG_DATA_HOME", "")]);
        assert_eq!(
            pref_path_for(PrefOs::Unix, &empty, "o", "a").as_deref(),
            Some("/home/j/.local/share/o/a/")
        );
        assert_eq!(pref_path_for(PrefOs::MacOs, &env(&[("HOME", "")]), "o", "a"), None);
    }

    #[test]
    fn resolve_honours_the_test_user_dir_then_the_pref_path() {
        let over = env(&[("HOME", "/Users/j"), ("OPENLIERO_TEST_USER_DIR", "/tmp/u")]);
        let store = NativeStore::resolve_with(PrefOs::MacOs, &over).expect("a store");
        assert_eq!(store.user_root(), Path::new("/tmp/u"));
        assert_eq!(store.system_root(), Some(Path::new(DATA_ROOT)));
        let pref = env(&[("HOME", "/Users/j")]);
        let store = NativeStore::resolve_with(PrefOs::MacOs, &pref).expect("a store");
        assert_eq!(
            store.user_root(),
            Path::new("/Users/j/Library/Application Support/openliero/openliero")
        );
        assert!(NativeStore::resolve_with(PrefOs::MacOs, &env(&[])).is_none());
    }

    #[test]
    fn native_reads_see_the_user_layer_over_the_system_layer() {
        let root = scratch("merged");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Setups/liero.cfg", b"system setup");
        put(&system, "Profiles/a.toml", b"system a");
        put(&user, "Profiles/a.toml", b"user a");
        let store = NativeStore::split(user, Some(system));
        assert_eq!(store.read("Setups/liero.cfg").as_deref(), Some(&b"system setup"[..]));
        assert_eq!(store.read("Profiles/a.toml").as_deref(), Some(&b"user a"[..]));
        assert_eq!(store.read("Profiles/none.toml"), None);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn native_writes_go_to_the_user_layer_only() {
        let root = scratch("write");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Setups/liero.cfg", b"system setup");
        let store = NativeStore::split(user.clone(), Some(system.clone()));
        store.write("Setups/liero.cfg", b"mine").expect("write creates Setups/");
        assert_eq!(std::fs::read(user.join("Setups/liero.cfg")).expect("user file"), b"mine");
        assert_eq!(
            std::fs::read(system.join("Setups/liero.cfg")).expect("system file"),
            b"system setup"
        );
        assert_eq!(store.read("Setups/liero.cfg").as_deref(), Some(&b"mine"[..]));
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn native_shadows_system_mirrors_paths_shadows_system() {
        let root = scratch("shadows");
        let (user, system) = (root.join("user"), root.join("system"));
        put(&system, "Profiles/Stock.toml", b"x");
        put(&user, "Profiles/Mine.toml", b"x");
        let split = NativeStore::split(user, Some(system.clone()));
        assert!(split.shadows_system("Setups", "liero.cfg"), "reserved");
        assert!(split.shadows_system("Setups", "LIERO.CFG"), "reserved, leaf case-insensitive");
        assert!(split.shadows_system("Profiles", "Stock.toml"), "the system layer has it");
        assert!(!split.shadows_system("Profiles", "Mine.toml"), "user-only files may be overwritten");
        assert!(!split.shadows_system("Profiles", "New.toml"));
        let single = NativeStore::single_dir(system.clone());
        assert!(single.shadows_system("Setups", "liero.cfg"), "reserved even in one directory");
        assert!(!single.shadows_system("Profiles", "Stock.toml"), "no separate layer to shadow");
        let same = NativeStore::split(system.clone(), Some(system));
        assert!(
            !same.shadows_system("Profiles", "Stock.toml"),
            "user root == system root (filesystem.cpp:754-759)"
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn paths_that_could_leave_the_root_are_refused() {
        let root = scratch("refuse");
        let store = NativeStore::single_dir(root.join("cfg"));
        for bad in ["", "/etc/passwd", "../x", "Setups/../../x", "Setups//x", "./x", "C:/x", "a\\b"] {
            assert_eq!(store.read(bad), None, "{bad:?}");
            let err = store.write(bad, b"x").expect_err(bad);
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        assert!(!root.join("x").exists() && !root.join("cfg").exists(), "nothing was written");
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn load_setup_reads_the_shipped_setup_through_the_system_layer() {
        let liero = shipped("Setups/liero.cfg");
        let store = MemoryStore::with_system(&[(SETUP_REL, liero.as_slice())]);
        assert_eq!(load_setup(&store).expect("load"), Settings::default());
        assert_eq!(store.user_file(SETUP_REL), None, "a readable setup is not rewritten");
    }

    #[test]
    fn load_setup_writes_the_defaults_when_the_setup_is_missing_or_broken() {
        let defaults = settings_to_toml(&Settings::default()).into_bytes();
        let broken: [Option<&[u8]>; 3] = [None, Some(b"[settings\nlives = "), Some(&[0xff, 0xfe])];
        for bytes in broken {
            let store = MemoryStore::new();
            if let Some(b) = bytes {
                store.write(SETUP_REL, b).expect("seed the user layer");
            }
            assert_eq!(load_setup(&store).expect("load"), Settings::default(), "{bytes:?}");
            assert_eq!(store.user_file(SETUP_REL), Some(defaults.clone()), "{bytes:?}");
        }
    }

    #[test]
    fn a_user_setup_shadows_the_shipped_one_and_save_setup_writes_the_cpp_bytes() {
        let liero = shipped("Setups/liero.cfg");
        let store = MemoryStore::with_system(&[(SETUP_REL, liero.as_slice())]);
        store.write(SETUP_REL, &shipped("Setups/orbmit.cfg")).expect("seed");
        let mut s = load_setup(&store).expect("load");
        assert_eq!(s.lives, 9, "the user layer's orbmit values win");
        s.map = false;
        save_setup(&store, &s).expect("save");
        assert_eq!(store.user_file(SETUP_REL), Some(settings_to_toml(&s).into_bytes()));
        assert!(!load_setup(&store).expect("reload").map);
    }

    #[test]
    fn memory_store_shadows_system_like_the_native_store() {
        let store = MemoryStore::with_system(&[("Profiles/Stock.toml", &b"x"[..])]);
        assert!(store.shadows_system("Setups", "Liero.CFG"));
        assert!(
            !store.shadows_system("setups", "liero.cfg"),
            "the subdir compare is exact (filesystem.cpp:744)"
        );
        assert!(store.shadows_system("Profiles", "Stock.toml"));
        assert!(!store.shadows_system("Profiles", "Mine.toml"));
    }

    #[test]
    fn load_setup_on_disk_creates_the_user_setup_only_when_needed() {
        let root = scratch("load_setup");
        let with_data = NativeStore::split(root.join("u1"), Some(PathBuf::from(DATA_ROOT)));
        assert_eq!(load_setup(&with_data).expect("load"), Settings::default());
        assert!(!root.join("u1/Setups/liero.cfg").exists(), "read from data/, nothing written");
        let bare = NativeStore::split(root.join("u2"), Some(root.join("empty")));
        assert_eq!(load_setup(&bare).expect("load"), Settings::default());
        assert_eq!(
            std::fs::read(root.join("u2/Setups/liero.cfg")).expect("the written setup"),
            settings_to_toml(&Settings::default()).into_bytes()
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib storage`
Expected: FAIL to compile — `cannot find value DATA_ROOT` / `cannot find type PrefOs` / `cannot find struct NativeStore`.

- [ ] **Step 2: Implement `paths.rs`** — above its test module:

```rust
//! Step 4½a-2 — the two compile-time data roots (design §2.2, §9.3.8), resolved relative to
//! this crate so every binary works from any CWD. `DATA_ROOT` is the read-only system layer
//! of `crate::storage::NativeStore` (the Rust analog of C++ `paths::SystemDataRoot`, the
//! binary-adjacent `data/`); `TC_ROOT` is the stock TC. Production code uses these; the
//! self-contained per-test `TC_ROOT` consts in `sim`/`oracle-tests`/… stay as they are.

/// The repository's `data/` directory.
pub const DATA_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");
/// The stock total conversion, `data/TC/openliero`.
pub const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
```

- [ ] **Step 3: Implement `storage.rs`** — above its test module:

```rust
//! Step 4½a-2 — settings storage (design §3.5, §9.3.7): the [`ConfigStore`] seam; the native
//! [`NativeStore`] mirroring C++ `paths::Resolve` (`filesystem.cpp:767-845`: reads see the
//! per-user directory layered over the read-only system `data/`, writes go to the user
//! directory only); the in-memory [`MemoryStore`] (unit tests, and the wasm stand-in until
//! 4½h's localStorage store implements the same trait); and [`load_setup`] / [`save_setup`]
//! (`gameEntry.cpp:55-58`, `:78`).
//!
//! A config path is a forward-slash name under the config root — `"Setups/liero.cfg"`,
//! `"Profiles/AI (L).toml"` — the C++ `configNode / "Setups" / "liero.cfg"`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::settings::Settings;
use crate::settings_toml::{settings_from_toml, settings_to_toml};

/// The setup the game reads at startup and writes at exit (`gameEntry.cpp:55`, `:78`).
pub const SETUP_REL: &str = "Setups/liero.cfg";
/// `paths::UserDataRoot`'s test-only override (`filesystem.cpp:658-667`).
pub const TEST_USER_DIR_ENV: &str = "OPENLIERO_TEST_USER_DIR";
/// The `SDL_GetPrefPath` organisation and application (`filesystem.cpp:669`).
pub const PREF_ORG: &str = "openliero";
pub const PREF_APP: &str = "openliero";

/// Where settings files are read from and written to.
pub trait ConfigStore {
    /// The file at `rel` through the merged view: the user layer, else the system layer.
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    /// Write `rel` into the user layer (never the system layer), creating parent directories.
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()>;
    /// `paths::ShadowsSystem`: would saving `subdir/leaf` clobber a reserved name or hide a
    /// shipped file? (4½e's Save As dialogs refuse such names.)
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool;
}

/// `ShadowsSystem`'s reserved names (`filesystem.cpp:740-747`): `Setups/liero.cfg`, the game's
/// own auto-write target — the subdir compared exactly, the leaf case-insensitively.
pub fn is_reserved(subdir: &str, leaf: &str) -> bool {
    subdir == "Setups" && leaf.eq_ignore_ascii_case("liero.cfg")
}

/// `rel` under `root`, or `None` for a path that is empty, absolute, drive-qualified,
/// backslashed, or has an empty, `.` or `..` component — nothing may leave the config root.
fn under(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return None;
    }
    let mut path = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

fn invalid(rel: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid config path {rel:?}"))
}

/// Which SDL3 `SDL_GetPrefPath` rule applies (`src/filesystem/{cocoa,unix,windows}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefOs {
    MacOs,
    Unix,
    Windows,
    Other,
}

impl PrefOs {
    /// The rule for the platform this was compiled for.
    pub fn current() -> PrefOs {
        if cfg!(target_os = "macos") {
            PrefOs::MacOs
        } else if cfg!(target_os = "windows") {
            PrefOs::Windows
        } else if cfg!(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        )) {
            PrefOs::Unix
        } else {
            PrefOs::Other
        }
    }
}

/// `SDL_GetPrefPath(org, app)` rebuilt from environment variables, with SDL's trailing
/// separator (design §9.3.7): macOS `$HOME/Library/Application Support/org/app/`; Linux/BSD
/// `$XDG_DATA_HOME/org/app/`, else `$HOME/.local/share/org/app/`; Windows
/// `%APPDATA%\org\app\`; anything else `None`. A missing or empty variable counts as unset.
pub fn pref_path_for(
    os: PrefOs,
    env: &dyn Fn(&str) -> Option<String>,
    org: &str,
    app: &str,
) -> Option<String> {
    let var = |key: &str| env(key).filter(|v| !v.is_empty());
    match os {
        PrefOs::MacOs => {
            let home = var("HOME")?;
            Some(format!(
                "{}/Library/Application Support/{org}/{app}/",
                home.trim_end_matches('/')
            ))
        }
        PrefOs::Unix => {
            let base = match var("XDG_DATA_HOME") {
                Some(xdg) => xdg.trim_end_matches('/').to_string(),
                None => format!("{}/.local/share", var("HOME")?.trim_end_matches('/')),
            };
            Some(format!("{base}/{org}/{app}/"))
        }
        PrefOs::Windows => {
            let appdata = var("APPDATA")?;
            Some(format!("{}\\{org}\\{app}\\", appdata.trim_end_matches('\\')))
        }
        PrefOs::Other => None,
    }
}

/// [`pref_path_for`] for this platform and the process environment.
pub fn pref_path(org: &str, app: &str) -> Option<PathBuf> {
    pref_path_for(PrefOs::current(), &|key: &str| std::env::var(key).ok(), org, app)
        .map(PathBuf::from)
}

/// The native store: `paths::Resolve`'s XDG split (`filesystem.cpp:833-845`) or one directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStore {
    user_root: PathBuf,
    system_root: Option<PathBuf>,
}

impl NativeStore {
    /// Reads: `user_root`, then `system_root`. Writes: `user_root`.
    pub fn split(user_root: PathBuf, system_root: Option<PathBuf>) -> NativeStore {
        NativeStore {
            user_root,
            system_root,
        }
    }

    /// One directory for reads and writes — the `--config-root` / `portable.txt` layout
    /// (`filesystem.cpp:811-816`, `:827-831`; both still deferred, design §9.3.7).
    pub fn single_dir(root: PathBuf) -> NativeStore {
        NativeStore::split(root, None)
    }

    /// `paths::Resolve` without flags: the per-user directory (`OPENLIERO_TEST_USER_DIR`, else
    /// `SDL_GetPrefPath("openliero", "openliero")`) over [`crate::paths::DATA_ROOT`]. `None`
    /// when no user directory can be determined.
    pub fn resolve_default() -> Option<NativeStore> {
        NativeStore::resolve_with(PrefOs::current(), &|key: &str| std::env::var(key).ok())
    }

    /// [`NativeStore::resolve_default`] over an explicit platform and environment.
    pub fn resolve_with(os: PrefOs, env: &dyn Fn(&str) -> Option<String>) -> Option<NativeStore> {
        let user = match env(TEST_USER_DIR_ENV).filter(|dir| !dir.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(pref_path_for(os, env, PREF_ORG, PREF_APP)?),
        };
        Some(NativeStore::split(
            user,
            Some(PathBuf::from(crate::paths::DATA_ROOT)),
        ))
    }

    pub fn user_root(&self) -> &Path {
        &self.user_root
    }

    pub fn system_root(&self) -> Option<&Path> {
        self.system_root.as_deref()
    }
}

impl ConfigStore for NativeStore {
    /// `FsNodeJoin::TryToReader` (`filesystem.cpp:372-378`): the user file if it reads, else
    /// the system one.
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(under(&self.user_root, rel)?)
            .ok()
            .or_else(|| std::fs::read(under(self.system_root.as_ref()?, rel)?).ok())
    }

    /// `FsNodeFilesystem::TryToWriter` (`filesystem.cpp:572-584`) on the user node: parent
    /// directories are created.
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        let path = under(&self.user_root, rel).ok_or_else(|| invalid(rel))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)
    }

    /// `paths::ShadowsSystem` (`filesystem.cpp:736-763`).
    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
        if is_reserved(subdir, leaf) {
            return true;
        }
        match &self.system_root {
            // Single-directory layouts have no separate layer to shadow (`:754-759`).
            Some(system) if *system != self.user_root => {
                under(system, &format!("{subdir}/{leaf}")).is_some_and(|p| p.exists())
            }
            _ => false,
        }
    }
}

/// An in-memory store: a writable user layer over a fixed system layer.
#[derive(Debug, Default)]
pub struct MemoryStore {
    user: RefCell<BTreeMap<String, Vec<u8>>>,
    system: BTreeMap<String, Vec<u8>>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore::default()
    }

    /// A store whose read-only system layer holds `files`.
    pub fn with_system(files: &[(&str, &[u8])]) -> MemoryStore {
        MemoryStore {
            user: RefCell::default(),
            system: files
                .iter()
                .map(|(rel, bytes)| (rel.to_string(), bytes.to_vec()))
                .collect(),
        }
    }

    /// The user layer's copy of `rel` (what a write left there), for tests.
    pub fn user_file(&self, rel: &str) -> Option<Vec<u8>> {
        self.user.borrow().get(rel).cloned()
    }
}

impl ConfigStore for MemoryStore {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        self.user
            .borrow()
            .get(rel)
            .cloned()
            .or_else(|| self.system.get(rel).cloned())
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        self.user.borrow_mut().insert(rel.to_string(), bytes.to_vec());
        Ok(())
    }

    fn shadows_system(&self, subdir: &str, leaf: &str) -> bool {
        is_reserved(subdir, leaf) || self.system.contains_key(&format!("{subdir}/{leaf}"))
    }
}

/// `gameEntry.cpp:55-58`: read `Setups/liero.cfg` through the merged view. When it is missing
/// or does not parse (invalid UTF-8 is a toml++ parse error too), fall back to
/// `Settings::default()` AND write those defaults to the user layer
/// (`gfx.SaveSettings(userConfigNode / "Setups" / "liero.cfg")`).
pub fn load_setup(store: &dyn ConfigStore) -> io::Result<Settings> {
    let parsed = store
        .read(SETUP_REL)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| settings_from_toml(&text).ok());
    match parsed {
        Some(s) => Ok(s),
        None => {
            let s = Settings::default();
            save_setup(store, &s)?;
            Ok(s)
        }
    }
}

/// `gameEntry.cpp:78`: `settings->save(userConfigNode / "Setups" / "liero.cfg")` — the C++
/// bytes ([`settings_to_toml`]) into the user layer.
pub fn save_setup(store: &dyn ConfigStore, s: &Settings) -> io::Result<()> {
    store.write(SETUP_REL, settings_to_toml(s).as_bytes())
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib storage` — Expected: PASS (12 tests).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p scenario --lib paths` — Expected: PASS (1 test).
Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/storage.rs` and the same for `paths.rs` — Expected: no output (else format those two new files and re-test).

- [ ] **Step 5: Switch the production `TC_ROOT` users** (hand edits; no rustfmt on `main.rs`/`lib.rs`)

In `rust/game/src/main.rs` delete `:35-37`:

```rust
/// TC asset root, resolved at compile time relative to this crate so `cargo run
/// -p game` works from any CWD (constraint: CARGO_MANIFEST_DIR, not CWD).
const TC_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
```

and directly under `use scenario::{Scenario, SceneData};` (`:25`) add:

```rust
// TC asset root, resolved at compile time so `cargo run -p game` works from any CWD —
// centralised in `scenario::paths` since Step 4½a-2.
use scenario::paths::TC_ROOT;
```

In `rust/shot/src/lib.rs` replace `:392-397`:

```rust
    let tc_root: PathBuf = cfg.tc_root.clone().unwrap_or_else(|| {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/TC/openliero"
        ))
    });
```

with:

```rust
    let tc_root: PathBuf = cfg
        .tc_root
        .clone()
        .unwrap_or_else(|| PathBuf::from(scenario::paths::TC_ROOT));
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p shot` — Expected: PASS (unchanged behaviour: same path).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS.
Run: `grep -rn "data/TC/openliero\"" /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/game/src/main.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/shot/src/lib.rs` — Expected: only `shot/src/lib.rs`'s `#[cfg(test)]` const (`:629` today) remains.

- [ ] **Step 6: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/scenario/src/paths.rs rust/scenario/src/storage.rs rust/scenario/src/lib.rs rust/game/src/main.rs rust/shot/src/lib.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "scenario(4.5a-2): ConfigStore seam, NativeStore (paths::Resolve) + MemoryStore, load/save_setup; TC_ROOT/DATA_ROOT centralised" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

---

### Task 5: The HUD fix — `game::hud_mode` + the binary draws the HUD  [Sonnet]

**Files:**
- Create: `rust/game/src/hud_mode.rs`
- Modify: `rust/game/src/lib.rs` (add `pub mod hud_mode;` after `pub mod audio;`)
- Modify: `rust/game/src/main.rs` — imports (`:23-31`), `struct Demo` (`:76-103`), `setup` (the `..` destructure comment `:398-400` and the `Demo` literal `:441-452`), `render_and_upload` (`:744-761`)

**Interfaces:**
- Consumes: `game::input::Mode` (`input.rs:144-149`), `render::frame::Scene` (`draw_hud`, `map` are `pub`, `frame.rs:51-54`), `scenario::Scenario::hud()` (`parser.rs:372`), T4's `scenario::paths::TC_ROOT`, 4½a-1's `Settings::default().map`.
- Produces:
  - `pub struct game::hud_mode::HudFlags { pub draw_hud: bool, pub map: bool }` (`Clone, Copy, Debug, PartialEq, Eq`) with `pub fn apply(self, scene: &mut render::frame::Scene<'_>)`.
  - `pub fn game::hud_mode::hud_flags(mode: Mode, scenario_hud: bool, settings_map: bool) -> HudFlags` — `Live`/`Replay` ⇒ `{ draw_hud: true, map: settings_map }`; `Scripted` ⇒ both = `scenario_hud`. (Design §8's tuple, named.)

Why: design §8, overview LD 9b. `SceneData::as_scene` returns a world-only `Scene`, and `render_and_upload` never flipped it, so the live binary showed no stats panel and no minimap. C++ always draws the HUD in play and gates the minimap on `settings->map` (`viewport.cpp:593`). `Scripted` keeps its scenario's `render_hud` directive, so the `blood` demo and its wasm frame-parity witness stay world-only. The `map` value is `Settings::default().map` until 4½d loads the setup (design §9.3.6: no config I/O from the binary in this slice).

- [ ] **Step 1: Write the failing tests** — create `rust/game/src/hud_mode.rs` containing ONLY this test module, and add `pub mod hud_mode;` to `game/src/lib.rs` after `pub mod audio;`:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use render::bitmap::Bitmap;
    use render::viewport::Viewport;
    use scenario::paths::TC_ROOT;
    use scenario::settings::Settings;
    use scenario::Scenario;

    use super::*;

    #[test]
    fn a_played_match_draws_the_hud_and_follows_the_map_setting() {
        for mode in [Mode::Live, Mode::Replay] {
            assert_eq!(hud_flags(mode, false, true), HudFlags { draw_hud: true, map: true });
            assert_eq!(hud_flags(mode, false, false), HudFlags { draw_hud: true, map: false });
            assert_eq!(
                hud_flags(mode, true, false),
                HudFlags { draw_hud: true, map: false },
                "a scenario directive does not override the setting"
            );
        }
        assert!(Settings::default().map, "the C++ default the binary passes until 4½d");
    }

    #[test]
    fn the_scripted_demo_keeps_its_scenario_directive() {
        assert_eq!(hud_flags(Mode::Scripted, false, true), HudFlags { draw_hud: false, map: false });
        assert_eq!(hud_flags(Mode::Scripted, true, false), HudFlags { draw_hud: true, map: true });
    }

    #[test]
    fn the_flags_reach_the_frame_of_the_live_default_match() {
        let scenario = Scenario::parse(include_str!("../scenarios/default_match.txt"))
            .expect("the default match parses");
        let loaded = scenario::load(Path::new(TC_ROOT), &scenario);
        let frame = |flags: HudFlags| -> Vec<u32> {
            let mut scene = loaded.scene.as_scene(0, scenario.shadow());
            flags.apply(&mut scene);
            let mut viewports = Viewport::player_layout();
            let mut bmp = Bitmap::new(320, 200);
            render::frame::draw(&mut bmp, &loaded.state, &mut viewports, &scene);
            bmp.pixels
        };
        let world_only = frame(hud_flags(Mode::Scripted, false, true));
        let hud = frame(hud_flags(Mode::Live, false, false));
        let hud_map = frame(hud_flags(Mode::Live, false, true));
        // Rows 158.. hold the stats panel and the minimap (render `frame.rs` HUD test).
        const HUD_START: usize = 158 * 320;
        assert_ne!(world_only[HUD_START..], hud[HUD_START..], "Live paints the stats panel");
        assert_ne!(hud[HUD_START..], hud_map[HUD_START..], "settings.map adds the minimap");
        assert_eq!(world_only[..HUD_START], hud_map[..HUD_START], "the world rows are untouched");
    }
}
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --lib hud_mode`
Expected: FAIL to compile — `cannot find function hud_flags in this scope` / `cannot find struct HudFlags`.

- [ ] **Step 2: Implement** — above the test module in `hud_mode.rs`:

```rust
//! Step 4½a-2 — which HUD the `game` binary draws (design §8, overview LD 9b: the live HUD
//! bug). `scenario::SceneData::as_scene` returns a world-only `Scene` (`draw_hud = map =
//! false`, kept for the 3a/3b goldens) and the binary never flipped it, so `cargo run -p game`
//! showed no stats panel and no minimap. C++ always draws the HUD during play and gates the
//! minimap on `settings->map` (`viewport.cpp:593`). A played match (`Live`) and a replay of
//! one (`Replay`) therefore draw `(true, settings.map)`. The `Scripted` demo keeps its
//! scenario's `render_hud` directive for both flags, so the `blood` demo — and the wasm
//! frame-parity witness built on it — stays world-only. Until 4½d loads the setup (design
//! §9.3.6) the binary passes `Settings::default().map`.

use render::frame::Scene;

use crate::input::Mode;

/// The two HUD switches of a `render::frame::Scene`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudFlags {
    pub draw_hud: bool,
    pub map: bool,
}

impl HudFlags {
    /// Set `scene.draw_hud` and `scene.map`.
    pub fn apply(self, scene: &mut Scene<'_>) {
        scene.draw_hud = self.draw_hud;
        scene.map = self.map;
    }
}

/// The HUD for `mode`: `Live`/`Replay` ⇒ drawn, minimap per `settings_map`; `Scripted` ⇒ the
/// scenario's `render_hud` directive (`scenario_hud`) for both.
pub fn hud_flags(mode: Mode, scenario_hud: bool, settings_map: bool) -> HudFlags {
    match mode {
        Mode::Live | Mode::Replay => HudFlags {
            draw_hud: true,
            map: settings_map,
        },
        Mode::Scripted => HudFlags {
            draw_hud: scenario_hud,
            map: scenario_hud,
        },
    }
}
```

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --lib hud_mode` — Expected: PASS (3 tests).
Run: `rustfmt --edition 2024 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/game/src/hud_mode.rs` — Expected: no output (else format it and re-test).

- [ ] **Step 3: Wire the binary** (hand edits in `main.rs`)

Imports — after `use game::match_flow::{FlowStep, MatchFlow};` add:

```rust
use game::hud_mode::{HudFlags, hud_flags};
use scenario::settings::Settings;
```

`struct Demo` — after the `flow: Option<MatchFlow>,` field add:

```rust
    /// Step 4½a-2 (live HUD bug, design §8): the HUD flags every draw applies — `Live`/`Replay`
    /// draw the stats panel + (per `settings.map`) the minimap; `Scripted` follows its
    /// scenario's `render_hud` directive. Fixed for the run (mode and scenario never change).
    hud: HudFlags,
```

`setup` — replace the destructure comment (`:398-400`):

```rust
        // `font`/`labels` (Slice 3e T0) are wired into the render path in T5; the
        // interactive `game` binary does not draw the HUD yet.
        ..
```

with:

```rust
        // `font`/`labels` travel inside `scene`; the HUD is drawn per `Demo.hud` (4½a-2).
        ..
```

and directly before `let mut demo = Demo {` add:

```rust
    // Step 4½a-2: the binary loads no setup yet (design §9.3.6), so the minimap follows the
    // C++ default (`map = true`); 4½d passes the loaded setup's `map` here.
    let hud = hud_flags(*mode, scenario.hud(), Settings::default().map);
```

and in the `Demo { … }` literal, after `flow: (*mode == Mode::Live).then(MatchFlow::new),` add `hud,`.

`render_and_upload` — replace:

```rust
    let scene = demo.scene.as_scene(sim.screen_flash, draw_shadow);
```

with:

```rust
    let mut scene = demo.scene.as_scene(sim.screen_flash, draw_shadow);
    // Step 4½a-2: draw the HUD the mode calls for (world-only for the Scripted demo).
    demo.hud.apply(&mut scene);
```

Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: builds, no new warnings.
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS (the pass-through determinism gate is state-only and unaffected).
Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --target wasm32-unknown-unknown` — Expected: builds (wasm is `Scripted`: world-only as before, so the debug frame-parity witness is untouched).
Manual (controller or John, optional): `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` shows the stats panel (health bars, lives/kills) and the minimap under the two viewports; `cargo run … -p game blood` still shows the world-only demo.

- [ ] **Step 4: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/game/src/hud_mode.rs rust/game/src/lib.rs rust/game/src/main.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "game(4.5a-2): the live binary draws the HUD + minimap (settings.map); Scripted stays world-only (live bug)" -m "Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

---

### Task 6: MILESTONE — Hard gate 5 green on the full corpus, `UpdateHash` vectors matched  [Opus]

**Files:**
- Modify: `rust/oracle-tests/tests/settings_toml_golden.rs` (append the milestone guards)

**Interfaces:**
- Consumes: T2's helpers (`corpus`, `entry`, `golden`, `read`, `REPO`, `Entry::*`), T3's `hashes`, all goldens.
- Produces: the milestone evidence for T7 and the PROGRESS entry.

Why: design §3.4 and §12's first risk. G5a–G5c are only as strong as the corpus. These guards make the corpus non-vacuous: every shipped file is in it, every quoting rule actually appears in the C++ bytes, every edge rule actually changed a loaded value, and the layout facts of design §9.3.3 hold on every output. The regeneration step proves the oracle deterministic.

- [ ] **Step 1: Write the guards** — append to `settings_toml_golden.rs`:

```rust
#[test]
fn milestone_the_corpus_covers_every_shipped_file_and_the_4_5a1_sidecars() {
    let entries = corpus();
    let paths: Vec<&str> = entries.iter().filter_map(|e| e.path.as_deref()).collect();
    for (dir, ext) in [("data/Setups", ".cfg"), ("data/Profiles", ".toml")] {
        let mut seen = 0;
        for file in std::fs::read_dir(Path::new(REPO).join(dir)).expect("shipped dir") {
            let name = file.expect("dir entry").file_name().into_string().expect("utf-8");
            if name.ends_with(ext) {
                seen += 1;
                let rel = format!("{dir}/{name}");
                assert!(paths.contains(&rel.as_str()), "{rel} is shipped but not in corpus.txt");
            }
        }
        assert!(seen > 0, "{dir} has shipped files");
    }
    for v in ["killemall", "scales", "gametag"] {
        let rel = format!("rust/oracle-tests/golden/sim_slice4_5a_{v}_setup.cfg");
        assert!(paths.contains(&rel.as_str()), "{rel} is in corpus.txt");
    }
    let count = |kind: &str| entries.iter().filter(|e| e.kind == kind).count();
    assert_eq!(
        (count("default-setup"), count("default-profile"), count("setup"), count("profile")),
        (1, 1, 9, 10)
    );
}

#[test]
fn milestone_cpp_cross_checks_and_layout() {
    // The shipped legacy liero.cfg loads as exactly Settings() (design §2.1): same bytes, hash.
    assert_eq!(golden("liero.cfg"), golden("defaults.cfg"));
    let h = hashes();
    assert_eq!(h["liero"], h["defaults"]);
    assert_ne!(h["orbmit"], h["defaults"], "orbmit changes four gameplay fields");
    assert_ne!(h["quoting"], h["defaults"], "tc and levelFile are in the gameplay subset");
    // The 4½a-1 generator wrote its sidecars in toml++'s canonical layout (T0 Step 5).
    for v in ["killemall", "scales", "gametag"] {
        let sidecar = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("golden/sim_slice4_5a_{v}_setup.cfg"));
        assert_eq!(golden(&format!("sidecar_{v}.cfg")), read(&sidecar), "sidecar {v}");
    }
    // Design §9.3.3: weapTable is the only multiline array; 3 worm tables x 5 inline arrays;
    // exactly one trailing newline.
    for e in corpus().iter().filter(|e| e.is_setup()) {
        let cfg = golden(&e.saved_name());
        assert!(cfg.contains("weapTable = [\n    "), "{}: weapTable is multiline", e.id);
        assert_eq!(cfg.matches(" = [ ").count(), 15, "{}: inline arrays", e.id);
        assert!(cfg.ends_with('\n') && !cfg.ends_with("\n\n"), "{}: one trailing newline", e.id);
    }
}

#[test]
fn milestone_every_quoting_rule_appears_in_the_cpp_bytes() {
    let cfg = golden("quoting.cfg");
    for want in [
        "tc = \"mix \\\"q\\\" 'a' \\\\x\"\n",
        "levelFile = \"del\\u007F\"\n",
        "name = \"O'Brien\"\n",
        "gamepadName = 'say \"hi\"'\n",
        "gamepadSerial = 'C:\\pads\\1'\n",
        "name = 'tab\there'\n",
        "gamepadName = 'Zo\u{e9}'\n",
        "gamepadSerial = \"a\\u2028b\"\n",
        "name = \"bell\\u0001esc\\u001B\"\n",
        "gamepadName = '''two\nlines'''\n",
        "gamepadSerial = '''it's\nmulti'''\n",
    ] {
        assert!(cfg.contains(want), "quoting.cfg lacks {want:?}");
    }
    let profile = golden("quoting_profile.toml");
    for want in [
        "name = \"cr\\rhere\"\n",
        "gamepadName = \"\"\"x\ny\\u0002\"\"\"\n",
        "gamepadSerial = '''back\\slash\nnext'''\n",
    ] {
        assert!(profile.contains(want), "quoting_profile.toml lacks {want:?}");
    }
    assert!(golden("edge_profile.toml").contains("name = \"nel\\u0085Zo\u{e9}\"\n"));
}

#[test]
fn milestone_edge_inputs_load_with_the_toml_input_archive_semantics() {
    // edge.cfg — every value below is also C++'s (G5a compared the saved bytes).
    let s = entry("edge").load_setup();
    assert_eq!(s.lives, 15, "4294967311 -> static_cast<int32_t> 15");
    assert_eq!(s.game_mode, u32::MAX);
    assert_eq!(s.select_bot_weapons, u32::MAX);
    assert_eq!(s.max_bonuses, -1, "i64::MAX -> int32 -1");
    assert_eq!(s.loading_time, -7);
    assert_eq!((s.blood, s.shadow, s.ai_traces), (100, true, false), "wrong types keep");
    assert_eq!(&s.weap_table[..4], &[2, 0, 1, 0], "positional; a string element keeps 0");
    let [p1, p2, np] = &s.worm_settings;
    assert_eq!(p1.name, "", "a wrongly typed name keeps ''");
    assert_eq!(p1.health, i32::MAX, "-2147483649 wraps");
    assert_eq!(p1.rgb, [252, 0, 4], "rgbDepth 7 => (v & 63) << 2; the 4th element ignored");
    assert_eq!(p1.weapons, [5, 6, 7, 8, 9]);
    assert_eq!(p1.controls, [1, 2, 32, 34, 29, 42, 56], "short array: the default tail stays");
    assert_eq!(p1.controls_ex, [u32::MAX; 8]);
    assert_eq!(p2.rgb, [240, 176, 240], "missing [player2]: the DEFAULT rgb is 6-bit expanded");
    assert_eq!(np.rgb, [255, 0, 70], "8-bit values clamp");
    assert_eq!(np.weapons, [1; 5], "an empty array keeps every slot");

    let m = entry("missing_tables").load_setup();
    assert_eq!(m.lives, 3);
    for (i, v) in m.weap_table.iter().enumerate() {
        assert_eq!(*v as usize, i % 3, "weapTable[{i}]; the 41st entry is ignored");
    }
    let rgbs: Vec<[i32; 3]> = m.worm_settings.iter().map(|ws| ws.rgb).collect();
    assert_eq!(rgbs, [[160, 160, 240], [240, 176, 240], [160, 160, 240]]);

    // array_player.cfg — design §3.2's positional reading, now confirmed by the real C++.
    let a = entry("array_player").load_setup();
    assert!(a.modern_colors && !a.record_replays, "slots 1 and 2 of `settings`");
    assert_eq!(a.lives, 15, "no slot left for lives");
    let p = &a.worm_settings[0];
    assert_eq!((p.name.as_str(), p.health, p.controller), ("Pos", 55, 7));
    assert_eq!((p.random_name, p.color, p.input_device), (false, 3, 1));
    assert_eq!((p.gamepad_name.as_str(), p.gamepad_serial.as_str()), ("g", "s"));
    assert_eq!((p.rgb, p.weapons), ([1, 2, 3], [9, 1, 1, 1, 1]));

    // edge_profile.toml — LoadProfile over a bare WormSettings().
    let q = entry("edge_profile").load_profile();
    assert_eq!(q.name, "nel\u{85}Zo\u{e9}");
    assert_eq!((q.health, q.controller, q.color), (100, 1, 0), "keep; 2^32+1 -> 1; restored");
    assert!(q.random_name, "a wrongly typed randomName keeps true");
    assert_eq!(q.weapons, [3, 1, 1, 1, 1]);
    assert_eq!(q.controls, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(q.gamepad_controls, [u32::MAX, 12, 13, 14, 110, 10, 0, 9]);
    assert_eq!(q.rgb, [252, 0, 128], "no rgbDepth => 6-bit expansion");
}
```

(If T0 Step 5 recorded a sidecar that is not byte-identical to its C++ save, delete the sidecar loop in `milestone_cpp_cross_checks_and_layout` and say why in the commit body.)

- [ ] **Step 2: Run the full gate**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test settings_toml_golden`
Expected: PASS (7 tests). Record the output line (`test result: ok. 7 passed`) for the PROGRESS entry.

- [ ] **Step 3: The oracle is deterministic**

Run: `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/gen_settings_golden.sh` — Expected: `oracle_dump_settings: 21 entries`.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 status --porcelain -- rust/oracle-tests/golden` — Expected: no output (the regeneration reproduced every committed golden, and no other golden moved).

- [ ] **Step 4: Format and commit**

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/settings_toml_golden.rs` — Expected: no output (else format and re-test).

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/settings_toml_golden.rs
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "oracle(4.5a-2): MILESTONE — settings byte gate green on the full corpus + UpdateHash vectors vs C++" -m "Corpus: 2 shipped setups, 8 shipped profiles, 3 sidecars, 6 synthetic inputs, 2 defaults (21 entries). G5a/G5b/G5c plus intent guards (coverage, quoting rules in the C++ bytes, edge semantics, layout); the oracle regenerates byte-identically." -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

Reviewer (Opus): the guards assert values, not tautologies (each edge value differs from its default); no golden edited by hand (`git log -p -- rust/oracle-tests/golden/settings` shows only T0's commit, or regenerations); Hard gate 5 as restated in design §3.4 is fully covered.

---

### Task 7: Full re-diff (DEBUG), wasm build, PROGRESS, overview and design status  [Opus review]

**Files:**
- Modify: `docs/superpowers/liero-rs-PROGRESS.md` (the "Last updated" header `:11-34`, the Step 4½ tree's 4½a entry `:745-752`, "Open for John" `:785-790`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md` (status line `:3`, the 4½a split bullet `:246-253`)
- Modify: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md` (status line `:3-4`; append to §9.3 any fact T1/T2/T6 found that contradicts §3.1/§3.2)

Re-read each file right before editing it (line numbers drift; a sibling slice may have edited PROGRESS/the overview).

- [ ] **Step 1: The full green board** (debug builds throughout)

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game` — Expected: PASS (every golden, incl. `settings_toml_golden` 7/7).
Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game` — Expected: PASS (incl. `hud_mode` 3/3 and the pass-through gate).
Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --target wasm32-unknown-unknown` — Expected: builds (`twox-hash` takes its scalar path on wasm32; `storage` compiles, unused, on wasm).
Run: `cargo tree --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim-core --depth 1` — Expected: `sim-core` with no dependencies.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --stat 03de202 -- rust/sim rust/sim-core` — Expected: no output (the sim is untouched).
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --name-status 03de202 -- rust/oracle-tests/golden` — Expected: only `A` lines, all under `rust/oracle-tests/golden/settings/` (39 files: `corpus.txt`, 6 `in/` inputs, 32 outputs) — plus, if the sibling 4½c-0 slice has landed in between, its own `A` lines; no `M` or `D` line anywhere.
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --stat 03de202 -- src CMakeLists.txt` — Expected: only `src/tools/oracle_dump/settings_dump.cpp` and `CMakeLists.txt` (+2 lines) from this slice.
Run: `uvx --from clang-format==22.1.0 clang-format --dry-run -Werror --style=file /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/src/tools/oracle_dump/settings_dump.cpp` — Expected: no output.
Run (the design §2.2 size rule — settings modules ≤ ~1 k non-test lines, else 4½e moves them into a `rust/settings` crate): `grep -n "^#\[cfg(test)\]" /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/settings.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/settings_toml.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/toml_fmt.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/storage.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/scenario/src/paths.rs` — each match's line number minus one is that file's non-test size; record the sum (expected ≈ 950). If it exceeds 1000, add "move `settings`/`settings_toml`/`toml_fmt`/`storage` into `rust/settings` (design §2.2)" to 4½e's line in PROGRESS; do not move anything now.

- [ ] **Step 2: Update PROGRESS** — set "Last updated" to the real current date (`date +%F`) and write a one-paragraph 4½a-2 summary: the toml++ formatter port + writer; the corpus-driven oracle `oracle_dump_settings`; Hard gate 5 green (21 entries — 2 shipped setups, 8 shipped profiles, 3 sidecars, 6 synthetic, 2 defaults — G5a/G5b/G5c, first run or the fixes needed); `UpdateHash` via `twox-hash =2.1.3`; `ConfigStore`/`NativeStore`/`MemoryStore`; `TC_ROOT`/`DATA_ROOT`; the live HUD; any T0 sidecar or T2 reader finding. Add that 4½a is complete. In the Step 4½ tree change the first 4½a line's `🔶` to `✅` and replace the line `│          4½a-2 ⬜: TOML writer + byte gate (vs C++ load+save) + UpdateHash + storage + HUD fix` with the block below, writing the `date +%F` output where it says `<date>`:

```
│          4½a-2 ✅ LANDED: toml++ formatter port + settings/profile writer, UpdateHash
│          (XXH3-64, twox-hash), ConfigStore + NativeStore (paths::Resolve) + MemoryStore,
│          TC_ROOT/DATA_ROOT, live HUD + minimap. 🎯 MILESTONE GREEN <date>: Hard gate 5 —
│          Rust load+save == C++ load+save on 21 corpus entries (new oracle_dump_settings),
│          C++-saved files round-trip, UpdateHash vectors match                  COMPLETE
```

In "Open for John", keep the restated-byte-gate item (design §11 Q3) but reword its tail to "… now implemented as G5a/G5b/G5c (4½a-2), awaiting John's OK on the reworded done-when".

- [ ] **Step 3: Update the overview** — status line: `**4½a-1 LANDED** (4½a split into a-1/a-2)` → `**4½a LANDED** (4½a-1 + 4½a-2)` and `4½a-2 + 4½c–4½h planned` → `4½c-0 + 4½c–4½h planned`. At the end of the 4½a split bullet's 4½a-2 sentence ("… + the HUD fix.") add: "**LANDED** (plan: `plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md`; the binary does no config I/O until 4½d — slice design §9.3.6)."

- [ ] **Step 4: Update the design** — status line `:3-4`: "4½a-1 plan written, 4½a-2 outlined" → "4½a-1 and 4½a-2 LANDED" (keep the rest), and add `plans/2026-09-10-liero-rs-step4.5-slice4.5a2-plan.md` next to the 4½a-1 plan on the "Companion plan" line. If T1/T2/T6 found a C++ behaviour differing from §3.1/§3.2, append it to §9.3 as a dated item (never rewrite §3).

- [ ] **Step 5: Broad review (Opus)** — re-read the whole 4½a-2 diff (`git -C … diff 03de202 --stat` then per file) against design §3.3–§3.5, §8, §9.2, §9.3. Check: every §3.1 rule has a formatter unit test AND a C++-byte guard; `gameplay_keys` = `SerializeGameplay`; the C++ tool calls the real load/save/`UpdateHash`; `storage` writes only under the user root and refuses escaping paths; the binary does no file I/O for settings; `Scripted`/wasm stay world-only; no `rust/sim*` change; no prior golden modified; no `*slice4.5c0*`/`*slice4_5c0*` file touched; no push, no PR. Bar: 0 Critical / 0 Important.

- [ ] **Step 6: Commit**

```
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add docs/superpowers/liero-rs-PROGRESS.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5a-match-config-and-settings-design.md
git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "docs(4.5a-2): PROGRESS + overview + design — slice 4.5a-2 landed, 4.5a complete" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_013v248FvUuRxjpbqS8K61Rz"
```

## Done-report (each task)

(a) What changed and why, (b) files touched, (c) tests run + risks. One commit per task on `liero-rs-step-4-5`; no push, no PR. The final report surfaces: the three `cmp` results and the hashes.txt KAT line (T0), whether G5a passed first run or needed a reader/writer fix and which (T2), the `cargo tree` evidence for `twox-hash` (T3), the milestone line `test result: ok. 7 passed` and the clean regeneration (T6), the settings-module size (T7), and confirmation that no pre-existing golden and nothing under `rust/sim*` changed.
