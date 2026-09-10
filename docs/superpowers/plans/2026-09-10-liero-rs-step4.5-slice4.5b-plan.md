# Step 4½, Slice 4½b — Random level generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port C++ `Level::GenerateRandom` (+ `GenerateDirtPattern`, `BlitStone`), `Level::MakeShadow` and `Level::GenerateFromSettings` into the Bevy-free `sim` crate, bit-exact against a new C++ oracle `oracle_dump_levelgen` — every stage's `material_id` hash and `rand.last`, over 3 seeds × 7 sizes × shadow on/off plus the file/fallback paths — and give 4½a's `CorrectShadow` port a function-level oracle (T9).

**Architecture:** One new `sim` module, `sim::levelgen`: the generator stages as `pub fn`s over `LevelSim`, composed by `generate_random`, plus `make_shadow`, dispatched by `generate_from_settings` → `assets::level::LevelData`. It takes `&mut Rand` and plain parameters (no `MatchConfig`, no I/O, no `SimState`). A new C++ dumper runs a self-checked stage replica of `GenerateRandom` built from the real primitives, plus a `CorrectShadow` dig stage, and writes `golden/levelgen.txt`; `oracle-tests/tests/levelgen_golden.rs` replays it stage by stage. `CorrectShadow` itself is ported by the parallel slice 4½a (`sim::shadow`); T9 runs after it lands.

**Tech Stack:** Rust 2021 (`sim`, `sim-core::rng::Rand` MT19937, `assets` sprite/tc/level parsers, `oracle-tests`), C++ (`game` library, CMake `OPENLIERO_BUILD_ORACLE_DUMP`, preset `macos-arm64`), FNV-1a 64 goldens.

**Spec:** `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-slice4.5b-level-generation-design.md` (cited **design §N**). Overview: `docs/superpowers/specs/2026-09-10-liero-rs-step4.5-game-shell-overview.md`.

## Global Constraints

- Determinism: no floats, no time, no hash-map iteration order in `sim`; integer-only, `wrapping_*` only where C++ wraps (`PalIdx` `+4`).
- All prior goldens byte-identical (`git diff --stat` over `rust/oracle-tests/golden/` shows only the new `levelgen.txt`).
- `SimState::new` signature unchanged (4½b does not touch `rust/sim/src/state.rs` at all).
- `sim-core` stays dependency-free (4½b does not touch `rust/sim-core/`).
- C++ changes confined to `src/tools/oracle_dump/` + its CMake block (`CMakeLists.txt:372-392`).
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`; never "Generated with Claude Code" in commits.
- Subagents must not `cd` and must use `git -C` (absolute paths everywhere).
- Bash discipline: one simple command per call; no `&&`, `;`, `$VAR`, heredocs, `>`/`>>` redirection or `find -exec` in agent commands (scripts on disk may use them). Create/modify files with the editor tools.
- File-disjoint from the parallel slice 4½a: do NOT touch `rust/scenario/`, `rust/sim/src/state.rs`, `rust/sim/src/shadow.rs` (4½a creates it), `rust/sim/src/blit.rs` (also a known rustfmt-drift file — never rustfmt it), or `src/tools/oracle_dump/sim_physics_dump.cpp`. Take plain parameters, never a `MatchConfig`.
- Do NOT push, do NOT open a PR (the controller owns both). No sub-subagents. rustfmt only the files this plan creates.
- Worktree: `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5` (branch `liero-rs-step-4-5`), abbreviated **WT** in prose only — commands spell the absolute path.

## Model tiers

- **[Opus]:** T0 (C++ oracle + self-checks), T1–T6 (bit-exact ports), T7 (milestone + divergence diagnosis), T9 (cross-slice oracle check).
- **[Sonnet]:** T8 (example, README, PROGRESS, re-diff) — mechanical.

## Parallelism

T0 touches only C++/script/golden files and runs in its **own worktree** in parallel with T1–T6 (the project's proven "disjoint files, cherry-pick" pattern). T7 needs T0's commit cherry-picked onto `liero-rs-step-4-5` and T1–T6 done. T8 closes the slice. **T9 is gated on 4½a:** it runs once 4½a's `sim::shadow::correct_shadow` (4½a plan T3) is on the branch; 4½b is complete without it, T9 only adds the function-level `CorrectShadow` check.

## File Structure

| Path | Action | Responsibility |
|---|---|---|
| `src/tools/oracle_dump/levelgen_dump.cpp` | Create (T0) | C++ oracle: stage replica of `GenerateRandom` + 3 self-checks vs the real functions, `MakeShadow`, dig stage with `CorrectShadow`, file cases, coverage guard |
| `CMakeLists.txt` | Modify `:390-391` (T0) | two lines: `oracle_dump_levelgen` target in the oracle block |
| `rust/oracle-tests/gen_levelgen_golden.sh` | Create (T0) | LOCAL/MANUAL golden regeneration |
| `rust/oracle-tests/golden/levelgen.txt` | Create (T0, generated) | the committed golden (42 `gen` + 5 `file` lines) |
| `rust/sim/src/levelgen.rs` | Create (T1–T6) | the generator: `new_level`, field, splats, stones, `blit_stone`, tunnels, formations, rocks, `generate_random`, `make_shadow`, `LevelGenParams`, `generate_from_settings`, `level_file_name` |
| `rust/sim/src/lib.rs` | Modify (T1) | `pub mod levelgen;` |
| `rust/oracle-tests/tests/levelgen_golden.rs` | Create (T7), extend (T9) | the differential test (MILESTONE); T9 adds the `shadow=1` dig-stage test via 4½a's `correct_shadow` |
| `rust/oracle-tests/examples/levelgen_snapshot.rs` | Create (T8) | eyeball BMP of a generated level |
| `rust/README.md` | Modify (T8) | "Levelgen golden" section |
| `docs/superpowers/liero-rs-PROGRESS.md` | Modify (T8) | 4½b status line |

---

### Task 0 [Opus]: C++ oracle `oracle_dump_levelgen` + gen script + committed golden (parallel worktree)

**Files:**
- Create: `src/tools/oracle_dump/levelgen_dump.cpp`
- Modify: `CMakeLists.txt:390-391` (insert after the `oracle_dump_lrp_gen` lines, before `endif()` at `:392`)
- Create: `rust/oracle-tests/gen_levelgen_golden.sh`
- Create (generated): `rust/oracle-tests/golden/levelgen.txt`

**Interfaces:**
- Consumes (C++, all existing): `Level::Resize/SetPixel/Pixel/Mat/GenerateDirtPattern/GenerateRandom/MakeShadow/GenerateFromSettings` (`level.hpp:35-80`, `:180`), `DrawDirtEffect`, `BlitStone`, `CorrectShadow` (`gfx/blit.hpp:51-55`), `stone_tab` (`common.hpp:30`), `Rand::Seed/operator()/last` (`rand.hpp:13-32`), `Common::load(FsNode)`.
- Produces: `rust/oracle-tests/golden/levelgen.txt`, format (design §10.5):
  - `gen <seed> <w> <h> <shadow> <field> <splats> <stones> <tunnels> <formations> <rocks> <shadowed> <dig> <form_count> <form_placed> <form_tries> <rock_count> <rock_placed> <rock_tries>`
  - `file <level_file> <seed> <shadow> <w> <h> <final>`
  - stage token = `%016llx:%08x` (FNV-1a 64 of `material_id` : `rand.last`); `<shadowed>` = `%016llx` or `-`.

- [ ] **Step 1: Create the isolated worktree**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 worktree add /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle -b liero-rs-step-4-5-levelgen-oracle liero-rs-step-4-5`
Expected: `Preparing worktree (new branch 'liero-rs-step-4-5-levelgen-oracle')`. All T0 paths below are inside this worktree (**OWT** = `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle`). (Alternatively the controller dispatches T0 with Agent `isolation: "worktree"`; then use that worktree's path.)

- [ ] **Step 2: Write the dumper**

Create `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/src/tools/oracle_dump/levelgen_dump.cpp`:

```cpp
// Generates the golden record for the Rust random-level-generation differential test
// (Step 4½, slice 4½b; rust/oracle-tests/tests/levelgen_golden.rs). Runs the REAL C++
// generator — Level::GenerateRandom (level.cpp:101-193), Level::MakeShadow (:195-216),
// Level::GenerateFromSettings (:397-429) — plus a stage-by-stage REPLICA of
// GenerateRandom whose only purpose is to emit intermediate hashes for diagnosis. The
// replica calls the real primitives (Level::SetPixel/Pixel/Mat, DrawDirtEffect, BlitStone,
// stone_tab) plus a verbatim copy of the file-static IsNoRock, and is SELF-CHECKED on every
// case against Level::GenerateDirtPattern, Level::GenerateRandom and
// Level::GenerateFromSettings (hash AND rand.last). Any mismatch exits 1, so a replica bug
// can never reach the golden: the replica slices the oracle's work, it is not the oracle.
//
// Output (argv[1]), one line per case:
//   gen <seed> <w> <h> <shadow> <field> <splats> <stones> <tunnels> <formations> <rocks>
//       <shadowed> <dig> <form_count> <form_placed> <form_tries> <rock_count>
//       <rock_placed> <rock_tries>
//   file <level_file> <seed> <shadow> <w> <h> <final>
// A stage token is "<fnv1a64(material_id) %016llx>:<rand.last %08x>". <shadowed> is the
// bare hash after MakeShadow (no RNG) or "-" when shadow == 0. <dig> = 12 dig-texture
// stamps (DrawDirtEffect texture 7) each followed, when shadow == 1, by CorrectShadow over
// the worm.cpp:931-934 rect, continuing from the post-generation RNG. tries = candidate
// positions drawn by the rock loops (loop iterations), summed.
//
// Run from the repo root (Common::load("data/TC/openliero") and the file cases' relative
// paths). Exits 1 unless some case hit the kMaxTries cap AND some uncapped case retried a
// rock placement (the matrix must exercise both loop exits). Built via
// OPENLIERO_BUILD_ORACLE_DUMP (see rust/oracle-tests/gen_levelgen_golden.sh). Not part of
// the default build.
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <string>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "gfx/blit.hpp"
#include "level.hpp"
#include "math/rect.hpp"
#include "rand.hpp"
#include "settings.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char const kB : data) {
    h ^= kB;
    h *= 0x100000001b3ULL;
  }
  return h;
}

[[noreturn]] void Fail(char const* what, uint32_t seed, int w, int h) {
  std::fprintf(stderr, "oracle_dump_levelgen: %s (seed %u, %dx%d)\n", what, seed, w, h);
  std::exit(1);
}

std::string HashOnly(Level const& level) {
  char buf[24];
  std::snprintf(buf, sizeof buf, "%016llx",
                static_cast<unsigned long long>(Fnv1a(level.material_id)));
  return buf;
}

// "<hash>:<last>" — the level hash and the RNG position after a stage.
std::string Stage(Level const& level, Rand const& rand) {
  char buf[32];
  std::snprintf(buf, sizeof buf, "%016llx:%08x",
                static_cast<unsigned long long>(Fnv1a(level.material_id)),
                static_cast<unsigned>(rand.last));
  return buf;
}

struct RockStats {
  int count = 0;
  int placed = 0;
  uint64_t tries = 0;
};

// Verbatim copy of the file-static IsNoRock (level.cpp:85-99).
bool IsNoRock(Level& level, int size, int x, int y) {
  Rect rect(x, y, x + size + 1, y + size + 1);
  rect.Intersect(Rect(0, 0, level.width, level.height));
  for (int yy = rect.y1; yy < rect.y2; ++yy) {
    for (int xx = rect.x1; xx < rect.x2; ++xx) {
      if (level.Mat(xx, yy).Rock()) {
        return false;
      }
    }
  }
  return true;
}

// level.cpp:12-26 — the diffusion noise field.
void Field(Common& common, Level& level, Rand& rand) {
  level.SetPixel(0, 0, rand(7) + 12, common);
  for (int y = 1; y < level.height; ++y) {
    level.SetPixel(0, y, ((rand(7) + 12) + level.Pixel(0, y - 1)) >> 1, common);
  }
  for (int x = 1; x < level.width; ++x) {
    level.SetPixel(x, 0, ((rand(7) + 12) + level.Pixel(x - 1, 0)) >> 1, common);
  }
  for (int y = 1; y < level.height; ++y) {
    for (int x = 1; x < level.width; ++x) {
      level.SetPixel(x, y, (level.Pixel(x - 1, y) + level.Pixel(x, y - 1) + rand(8) + 12) / 3,
                     common);
    }
  }
}

// level.cpp:30-71 — rand(100) large-sprite splats with the 177..179 blend.
void Splats(Common& common, Level& level, Rand& rand) {
  int const kCount = rand(100);
  for (int i = 0; i < kCount; ++i) {
    int const kX = rand(level.width) - 8;
    int const kY = rand(level.height) - 8;
    int const kTemp = rand(4) + 69;
    PalIdx const* image = common.large_sprites.SpritePtr(kTemp);
    for (int cy = 0; cy < 16; ++cy) {
      int const kMy = cy + kY;
      if (kMy >= level.height) {
        break;
      }
      if (kMy < 0) {
        continue;
      }
      for (int cx = 0; cx < 16; ++cx) {
        int const kMx = cx + kX;
        if (kMx >= level.width) {
          break;
        }
        if (kMx < 0) {
          continue;
        }
        PalIdx const kSrcPix = image[(cy << 4) + cx];
        if (kSrcPix > 0) {
          PalIdx const kPix = level.Pixel(kMx, kMy);
          if (kPix > 176 && kPix < 180) {
            level.SetPixel(kMx, kMy, (kSrcPix + kPix) / 2, common);
          } else {
            level.SetPixel(kMx, kMy, kSrcPix, common);
          }
        }
      }
    }
  }
}

// level.cpp:73-82 — rand(15) stones.
void Stones(Common& common, Level& level, Rand& rand) {
  int const kCount = rand(15);
  for (int i = 0; i < kCount; ++i) {
    int const kX = rand(level.width) - 8;
    int const kY = rand(level.height) - 8;
    int const kWhich = rand(4) + 56;
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(kWhich), kX, kY);
  }
}

// level.cpp:108-135 — dirt-effect worm tunnels.
void Tunnels(Common& common, Level& level, Rand& rand) {
  int const kCount = rand(50) + 5;
  for (int i = 0; i < kCount; ++i) {
    int cx = rand(level.width) - 8;
    int cy = rand(level.height) - 8;
    int const kDx = rand(11) - 5;
    int const kDy = rand(5) - 2;
    int const kCount2 = rand(12);
    for (int j = 0; j < kCount2; ++j) {
      int const kCount3 = rand(5);
      for (int k = 0; k < kCount3; ++k) {
        cx += kDx;
        cy += kDy;
        DrawDirtEffect(common, rand, level, 1, cx, cy);
      }
      cx -= (kCount3 + 1) * kDx;
      cy -= (kCount3 + 1) * kDy;
      cx += rand(7) - 3;
      cy += rand(15) - 7;
    }
  }
}

// level.cpp:137-170 — 32x32 rock formations with the kMaxTries cap.
RockStats Formations(Common& common, Level& level, Rand& rand) {
  RockStats s;
  int const kMaxTries = level.width * level.height;
  s.count = rand(15) + 5;
  for (int i = 0; i < s.count; ++i) {
    int cx = 0;
    int cy = 0;
    int tries = 0;
    do {
      ++s.tries;
      cx = rand(level.width) - 16;
      if (rand(4) == 0) {
        cy = level.height - 1 - rand(20);
      } else {
        cy = rand(level.height) - 16;
      }
    } while (!IsNoRock(level, 32, cx, cy) && ++tries < kMaxTries);
    if (tries >= kMaxTries) {
      continue;
    }
    ++s.placed;
    int const kRock = rand(3);
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(stone_tab[kRock][0]), cx,
              cy);
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(stone_tab[kRock][1]),
              cx + 16, cy);
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(stone_tab[kRock][2]), cx,
              cy + 16);
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(stone_tab[kRock][3]),
              cx + 16, cy + 16);
  }
  return s;
}

// level.cpp:172-192 — 16x16 rocks, same cap.
RockStats Rocks(Common& common, Level& level, Rand& rand) {
  RockStats s;
  int const kMaxTries = level.width * level.height;
  s.count = rand(25) + 5;
  for (int i = 0; i < s.count; ++i) {
    int cx = 0;
    int cy = 0;
    int tries = 0;
    do {
      ++s.tries;
      cx = rand(level.width) - 8;
      if (rand(5) == 0) {
        cy = level.height - 1 - rand(13);
      } else {
        cy = rand(level.height) - 8;
      }
    } while (!IsNoRock(level, 15, cx, cy) && ++tries < kMaxTries);
    if (tries >= kMaxTries) {
      continue;
    }
    ++s.placed;
    BlitStone(common, level, /*p1=*/false, common.large_sprites.SpritePtr(rand(6) + 3), cx, cy);
  }
  return s;
}

// 12 dig stamps (worm.cpp:931-934 shape) continuing the generation RNG. The -7 and the
// modulo put the first stamps across the top/left edges (clip paths).
void Dig(Common& common, Level& level, Rand& rand, bool shadow) {
  for (int i = 0; i < 12; ++i) {
    int const kX = ((i * 41) + 5) % level.width - 7;
    int const kY = ((i * 29) + 7) % level.height - 7;
    DrawDirtEffect(common, rand, level, 7, kX, kY);
    if (shadow) {
      CorrectShadow(common, level, Rect(kX - 3, kY - 3, kX + 18, kY + 18));
    }
  }
}

struct Coverage {
  bool capped = false;
  bool retried = false;
};

Coverage DumpGen(std::FILE* out, Common& common, uint32_t seed, int w, int h, bool shadow) {
  Settings settings;
  settings.random_level = true;
  settings.random_map_width = w;
  settings.random_map_height = h;
  settings.shadow = shadow;

  Level level(common);
  Rand rand;
  rand.Seed(seed);
  level.Resize(w, h);

  Field(common, level, rand);
  std::string const kField = Stage(level, rand);
  Splats(common, level, rand);
  std::string const kSplats = Stage(level, rand);
  Stones(common, level, rand);
  std::string const kStones = Stage(level, rand);
  Tunnels(common, level, rand);
  std::string const kTunnels = Stage(level, rand);
  RockStats const kForm = Formations(common, level, rand);
  std::string const kFormations = Stage(level, rand);
  RockStats const kRock = Rocks(common, level, rand);
  std::string const kRocks = Stage(level, rand);

  {  // Self-check 1: the first three stages == Level::GenerateDirtPattern.
    Level real(common);
    Rand rr;
    rr.Seed(seed);
    real.Resize(w, h);
    real.GenerateDirtPattern(common, rr);
    if (Stage(real, rr) != kStones) {
      Fail("replica != Level::GenerateDirtPattern", seed, w, h);
    }
  }
  {  // Self-check 2: all six stages == Level::GenerateRandom.
    Level real(common);
    Rand rr;
    rr.Seed(seed);
    real.GenerateRandom(common, settings, rr);
    if (Stage(real, rr) != kRocks) {
      Fail("replica != Level::GenerateRandom", seed, w, h);
    }
  }

  std::string shadowed = "-";
  if (shadow) {
    level.MakeShadow(common);
    shadowed = HashOnly(level);
  }
  {  // Self-check 3: generation (+ MakeShadow) == Level::GenerateFromSettings.
    Level real(common);
    Rand rr;
    rr.Seed(seed);
    real.GenerateFromSettings(common, settings, rr);
    if (Stage(real, rr) != Stage(level, rand)) {
      Fail("replica != Level::GenerateFromSettings", seed, w, h);
    }
  }

  Dig(common, level, rand, shadow);
  std::string const kDig = Stage(level, rand);

  std::fprintf(out,
               "gen %u %d %d %d %s %s %s %s %s %s %s %s %d %d %" PRIu64 " %d %d %" PRIu64 "\n",
               seed, w, h, shadow ? 1 : 0, kField.c_str(), kSplats.c_str(), kStones.c_str(),
               kTunnels.c_str(), kFormations.c_str(), kRocks.c_str(), shadowed.c_str(),
               kDig.c_str(), kForm.count, kForm.placed, kForm.tries, kRock.count, kRock.placed,
               kRock.tries);

  Coverage c;
  c.capped = kForm.placed < kForm.count || kRock.placed < kRock.count;
  c.retried = !c.capped && (kForm.tries > static_cast<uint64_t>(kForm.count) ||
                            kRock.tries > static_cast<uint64_t>(kRock.count));
  return c;
}

void DumpFile(std::FILE* out, Common& common, char const* level_file, uint32_t seed,
              bool shadow) {
  Settings settings;
  settings.random_level = false;
  settings.level_file = level_file;
  settings.shadow = shadow;  // random_map_width/height stay 504x350 for the fallback
  Level level(common);
  Rand rand;
  rand.Seed(seed);
  level.GenerateFromSettings(common, settings, rand);
  std::fprintf(out, "file %s %u %d %d %d %s\n", level_file, seed, shadow ? 1 : 0, level.width,
               level.height, Stage(level, rand).c_str());
}

struct Size {
  int w;
  int h;
};

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: oracle_dump_levelgen <out.txt>   (run from the repo root)\n");
    return 1;
  }
  auto common = std::make_shared<Common>();
  FsNode const kTcRoot(FsNode("data") / "TC" / "openliero");
  common->load(kTcRoot);

  std::FILE* out = std::fopen(argv[1], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[1]);
    return 1;
  }

  // The matrix (design §10.3). The Rust test iterates the golden, so this list is the
  // single source of truth; if a coverage guard below fails, add seeds here.
  static constexpr uint32_t kSeeds[] = {1U, 42U, 2654435769U};
  static constexpr Size kSizes[] = {{504, 350}, {600, 350}, {64, 64},  {128, 96},
                                    {101, 77},  {2000, 72}, {72, 1000}};
  int capped = 0;
  int retried = 0;
  for (uint32_t const kSeed : kSeeds) {
    for (Size const kSize : kSizes) {
      for (int shadow = 0; shadow < 2; ++shadow) {
        Coverage const kC = DumpGen(out, *common, kSeed, kSize.w, kSize.h, shadow != 0);
        capped += kC.capped ? 1 : 0;
        retried += kC.retried ? 1 : 0;
      }
    }
  }
  DumpFile(out, *common, "data/TC/openliero/Levels/see_shadow_test.lev", 1U, true);
  DumpFile(out, *common, "data/TC/openliero/Levels/see_shadow_test.lev", 1U, false);
  DumpFile(out, *common, "data/TC/openliero/Levels/render_stage.lev", 1U, true);
  DumpFile(out, *common, "data/TC/openliero/Levels/does_not_exist", 42U, true);
  DumpFile(out, *common, "data/TC/openliero/Levels/does_not_exist", 42U, false);
  std::fclose(out);

  std::fprintf(stderr, "oracle_dump_levelgen: %d capped cases, %d retried cases\n", capped,
               retried);
  if (capped == 0 || retried == 0) {
    std::fprintf(stderr, "oracle_dump_levelgen: matrix misses the cap or the retry path\n");
    return 1;
  }
  return 0;
}
```

- [ ] **Step 3: Register the target**

In `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/CMakeLists.txt`, replace

```cmake
  add_executable(oracle_dump_lrp_gen src/tools/oracle_dump/lrp_gen.cpp)
  target_link_libraries(oracle_dump_lrp_gen PRIVATE game)
endif()
```

with

```cmake
  add_executable(oracle_dump_lrp_gen src/tools/oracle_dump/lrp_gen.cpp)
  target_link_libraries(oracle_dump_lrp_gen PRIVATE game)
  add_executable(oracle_dump_levelgen src/tools/oracle_dump/levelgen_dump.cpp)
  target_link_libraries(oracle_dump_levelgen PRIVATE game)
endif()
```

- [ ] **Step 4: Write the gen script**

Create `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/gen_levelgen_golden.sh`:

```bash
#!/usr/bin/env bash
# Regenerates golden/levelgen.txt by running the REAL C++ level generator
# (Level::GenerateRandom / MakeShadow / GenerateFromSettings, plus a self-checked stage
# replica and a CorrectShadow dig stage — see src/tools/oracle_dump/levelgen_dump.cpp).
# Needs the full C++ build (links the `game` target), so this is a LOCAL/MANUAL step — it
# is NOT run in the lightweight rust.yml CI. Override PRESET for other platforms (e.g.
# linux-x64). Unlike the older gen_*.sh it cd's to ROOT first, so it works from any cwd.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRESET="${PRESET:-macos-arm64}"
cd "$ROOT"
cmake --preset "$PRESET" -DOPENLIERO_BUILD_ORACLE_DUMP=ON >/dev/null
cmake --build "$ROOT/build/$PRESET" --config Release --target oracle_dump_levelgen
# Run from ROOT so Common::load("data/TC/openliero") and the file cases resolve.
"build/$PRESET/Release/oracle_dump_levelgen" "rust/oracle-tests/golden/levelgen.txt"
echo "wrote rust/oracle-tests/golden/levelgen.txt"
```

Run: `chmod +x /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/gen_levelgen_golden.sh`
Expected: no output.

- [ ] **Step 5: Run the oracle (the self-checks are this task's test)**

Run (Bash `run_in_background: true` — a fresh worktree's first configure bootstraps vcpkg and builds `game`, which can exceed 10 min): `bash /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/gen_levelgen_golden.sh`
Expected tail: `oracle_dump_levelgen: <N> capped cases, <M> retried cases` with N ≥ 1 and M ≥ 1, then `wrote rust/oracle-tests/golden/levelgen.txt`; exit 0.

If it exits with `replica != Level::GenerateDirtPattern` / `GenerateRandom` / `GenerateFromSettings`: the **replica** is wrong (never edit `level.cpp`) — diff the named function against the replica statement by statement (`level.cpp:11-193`, `:397-429`) and rerun. If it exits with `matrix misses the cap or the retry path`: add seeds to `kSeeds` (e.g. `7U`, `99U`) and rerun until both counts are ≥ 1; the Rust test (T7) is keyed on the golden, not on the seed list, and asserts only the 14 (size, shadow) combinations and the 7 file lines (amended 2026-09-10, controller ruling, T0 review: includes the `levelgen_shadow_fixture.lev` MakeShadow pair).

- [ ] **Step 6: Sanity-check the golden**

Run: `grep -c "^gen " /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/golden/levelgen.txt`
Expected: `42` (or `14 × number of seeds` if Step 5 added seeds).

Run: `grep "^file " /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/golden/levelgen.txt`
Expected: 5 lines; the two `see_shadow_test.lev` lines and the `render_stage.lev` line end in `:00000000` (file branch, no RNG) and report `504 350`; the two `does_not_exist` lines end in a non-zero `rand.last` (fallback ran) and report `504 350`.

Run: `grep -c " - " /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/rust/oracle-tests/golden/levelgen.txt`
Expected: `21` (one `-` `<shadowed>` column per `shadow=0` gen line; `7 × seeds` in general).

- [ ] **Step 7: Format check (CI runs clang-format tree-wide, pinned to v22)**

Run: `clang-format -i /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/src/tools/oracle_dump/levelgen_dump.cpp`
Then: `clang-format --dry-run -Werror /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle/src/tools/oracle_dump/levelgen_dump.cpp`
Expected: no output, exit 0. (Use `clang-format-22` if the default binary is another major version; if no v22 is installed, say so in the commit message for the controller.) If `-i` changed the file, rerun Step 5 so the committed golden comes from the formatted source (formatting cannot change behaviour, but the rerun is the proof).

- [ ] **Step 8: Commit (one commit — the controller cherry-picks it)**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle add src/tools/oracle_dump/levelgen_dump.cpp CMakeLists.txt rust/oracle-tests/gen_levelgen_golden.sh rust/oracle-tests/golden/levelgen.txt`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle commit -m "oracle(4½b): oracle_dump_levelgen + gen script + levelgen golden" -m "Self-checked stage replica of Level::GenerateRandom (field/splats/stones/tunnels/formations/rocks), MakeShadow, CorrectShadow dig stage, GenerateFromSettings file + fallback cases. <N> capped / <M> retried cases; <G> gen + 7 file lines." -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
(amended 2026-09-10, controller ruling, T0 review: file-line count raised from 5 to 7 — a `levelgen_shadow_fixture.lev` MakeShadow pair was added.)
(Fill `<N>`, `<M>`, `<G>` with the numbers Step 5/6 printed.)
Expected: `4 files changed`.

- [ ] **Step 9 (controller, before T7): bring the oracle onto the slice branch**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 cherry-pick liero-rs-step-4-5-levelgen-oracle`
Expected: a clean cherry-pick (T1–T6 touch no T0 file).
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 worktree remove /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5-levelgen-oracle`
(Keep the branch until the PR merges; its `build/` dir goes with the worktree.)

---

### Task 1 [Opus]: `sim::levelgen` scaffold — `new_level` + the dirt noise field

**Files:**
- Create: `rust/sim/src/levelgen.rs`
- Modify: `rust/sim/src/lib.rs:18` (add `pub mod levelgen;` after `pub mod hash;`)
- Test: `rust/sim/src/levelgen.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `sim_core::rng::Rand` (`new`, `seed(u32)`, `bound(u32) -> u32`, `last() -> u32`, `draws() -> u64`; `rust/sim-core/src/rng.rs:48-139`); `sim::state::LevelSim { width: i32, height: i32, material_id: Vec<u8>, material_flags: [u8; 256] }` with `set_material(&mut self, idx: usize, v: u8)` (`state.rs:640-645`, `:776-778`); `sim::state::MAT_ROCK`.
- Produces:
  - `pub fn new_level(width: i32, height: i32, material_flags: &[u8; 256]) -> LevelSim`
  - `pub fn generate_dirt_field(level: &mut LevelSim, rand: &mut Rand)`

- [ ] **Step 1: Write the failing tests**

Create `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`:

```rust
//! Random level generation — port of C++ `Level::GenerateRandom` and friends
//! (`src/game/level.cpp:11-193`, `:397-429`) plus `BlitStone` (`gfx/blit.cpp:462-532`).
//! Step 4½, slice 4½b.
//!
//! Bit-exact vs C++ for the same `Rand` state, TC assets and dimensions — gated by
//! `oracle-tests/tests/levelgen_golden.rs` against `oracle_dump_levelgen`, which hashes
//! `material_id` and `rand.last` after every stage. Integer-only; no I/O; no `SimState`.
//!
//! * **The generator never constructs a `Rand`.** Every entry point takes `&mut Rand`; the
//!   game shell seeds a dedicated one from the match seed (overview locked decision 6,
//!   design §2), the oracle drives it from any seeded state.
//! * **Flags are read live** from `material_flags[material_id[i]]` — equivalent to the C++
//!   `materials[]` cache because every generation writer keeps that cache in sync and the
//!   field writes every cell before any flag is read (design F6).
//! * **C++ `rand()` returns `u32`**: `rand(n) - 8`, `height - 1 - rand(20)` wrap and convert
//!   back to `int`, which is exactly `rand.bound(n) as i32 - 8` here (`bound < n <= 4096`,
//!   design F7). No C++ expression here holds two `rand()` calls, so there is no
//!   unspecified evaluation order to reproduce.

use sim_core::rng::Rand;

use crate::state::LevelSim;

/// Largest level side C++ accepts (`level.cpp:232`, `assets/src/level.rs:51`).
const MAX_DIM: i32 = 4096;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MAT_ROCK;

    fn seeded(seed: u32) -> Rand {
        let mut r = Rand::new();
        r.seed(seed);
        r
    }

    #[test]
    fn new_level_is_zero_filled_with_dims_and_flags() {
        let mut flags = [0u8; 256];
        flags[7] = MAT_ROCK;
        let level = new_level(5, 3, &flags);
        assert_eq!((level.width, level.height), (5, 3));
        assert_eq!(level.material_id, vec![0u8; 15]);
        assert_eq!(level.material_flags, flags);
    }

    /// level.cpp:12-26 restated statement by statement with an identically seeded
    /// reference `Rand`: corner, then COLUMN x=0 (y=1..h), then ROW y=0 (x=1..w), then the
    /// interior row-major. Swapping the column and row loops shifts every later draw.
    #[test]
    fn dirt_field_follows_level_cpp_12_26_draw_order() {
        let (w, h) = (9i32, 7i32);
        let mut r = seeded(7);
        let mut want = vec![0u8; (w * h) as usize];
        let at = |x: i32, y: i32| (x + y * w) as usize;
        want[0] = (r.bound(7) + 12) as u8;
        for y in 1..h {
            want[at(0, y)] = ((r.bound(7) + 12 + want[at(0, y - 1)] as u32) >> 1) as u8;
        }
        for x in 1..w {
            want[at(x, 0)] = ((r.bound(7) + 12 + want[at(x - 1, 0)] as u32) >> 1) as u8;
        }
        for y in 1..h {
            for x in 1..w {
                let left = want[at(x - 1, y)] as u32;
                let up = want[at(x, y - 1)] as u32;
                want[at(x, y)] = ((left + up + r.bound(8) + 12) / 3) as u8;
            }
        }

        let mut level = new_level(w, h, &[0u8; 256]);
        let mut rand = seeded(7);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(level.material_id, want);
        assert_eq!(rand.last(), r.last());
        assert_eq!(rand.draws(), r.draws());
    }

    #[test]
    fn dirt_field_draws_w_times_h_and_stays_in_12_to_18() {
        let mut level = new_level(17, 9, &[0u8; 256]);
        let mut rand = seeded(42);
        generate_dirt_field(&mut level, &mut rand);
        assert_eq!(rand.draws(), 17 * 9);
        assert!(level.material_id.iter().all(|&p| (12..=18).contains(&p)));
    }
}
```

In `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/lib.rs` replace

```rust
pub mod hash;
pub mod nobject;
```

with

```rust
pub mod hash;
pub mod levelgen;
pub mod nobject;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile error `error[E0425]: cannot find function `new_level` in this scope` (and `generate_dirt_field`).

- [ ] **Step 3: Implement**

In `rust/sim/src/levelgen.rs`, insert between `const MAX_DIM` and `#[cfg(test)]`:

```rust
/// `Level::Resize` (`level.cpp:218-227`) on a fresh level: a zero-filled `width × height`
/// material map carrying the TC flag table. Precondition `1 <= width, height <= 4096`
/// (the menu range is 64..=4096 step 8, `gfx.cpp:1295-1297`, but a hand-edited setup file
/// can hold any value).
pub fn new_level(width: i32, height: i32, material_flags: &[u8; 256]) -> LevelSim {
    debug_assert!(
        (1..=MAX_DIM).contains(&width) && (1..=MAX_DIM).contains(&height),
        "level size {width}x{height} outside 1..=4096"
    );
    LevelSim {
        width,
        height,
        material_id: vec![0u8; (width * height) as usize],
        material_flags: *material_flags,
    }
}

/// The diffusion noise field — first block of `GenerateDirtPattern` (`level.cpp:12-26`).
/// Draws exactly `width * height` values: the corner `rand(7)+12`; the first COLUMN, each
/// `(rand(7)+12 + above) >> 1`; the first ROW, each `(rand(7)+12 + left) >> 1`; the
/// interior row-major, each `(left + up + rand(8)+12) / 3`. Values stay in 12..=18 (the
/// tc.cfg dirt shades).
pub fn generate_dirt_field(level: &mut LevelSim, rand: &mut Rand) {
    let w = level.width;
    let h = level.height;
    let at = |x: i32, y: i32| (x + y * w) as usize;
    let corner = rand.bound(7) + 12;
    level.set_material(0, corner as u8);
    for y in 1..h {
        let up = level.material_id[at(0, y - 1)] as u32;
        let v = (rand.bound(7) + 12 + up) >> 1;
        level.set_material(at(0, y), v as u8);
    }
    for x in 1..w {
        let left = level.material_id[at(x - 1, 0)] as u32;
        let v = (rand.bound(7) + 12 + left) >> 1;
        level.set_material(at(x, 0), v as u8);
    }
    for y in 1..h {
        for x in 1..w {
            let left = level.material_id[at(x - 1, y)] as u32;
            let up = level.material_id[at(x, y - 1)] as u32;
            let v = (left + up + rand.bound(8) + 12) / 3;
            level.set_material(at(x, y), v as u8);
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 3 passed; 0 failed`.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (if it prints a diff, run it without `--check` — this file is new, so formatting it is allowed).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs rust/sim/src/lib.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): levelgen scaffold + GenerateDirtPattern noise field" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `2 files changed`.

---

### Task 2 [Opus]: `blit_stone`, large-sprite splats, stones, `generate_dirt_pattern`

**Files:**
- Modify: `rust/sim/src/levelgen.rs` (imports; new fns after `generate_dirt_field`; tests appended inside `mod tests`)

**Interfaces:**
- Consumes: T1's `new_level`, `generate_dirt_field`; `assets::sprite::SpriteSet { width, height, count, data }` + `sprite(&self, frame: usize) -> &[u8]` (`assets/src/sprite.rs:27-32`, `:132-135`).
- Produces:
  - `pub fn blit_stone(level: &mut LevelSim, large_sprites: &SpriteSet, frame: usize, x: i32, y: i32)`
  - `pub fn splat_large_sprites(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand)`
  - `pub fn scatter_stones(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand)`
  - `pub fn generate_dirt_pattern(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand)`
  - private `fn splat_sprite(level: &mut LevelSim, image: &[u8], kx: i32, ky: i32)`
  - test helper `fn bank(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet` (used by T3, T4)

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` (after the last test) in `rust/sim/src/levelgen.rs`:

```rust
    /// A bank of `count` 16x16 sprites, all zero except the `(frame, 256 bytes)` overrides.
    fn bank(count: i32, overrides: &[(usize, Vec<u8>)]) -> SpriteSet {
        let mut data = vec![0u8; count as usize * 256];
        for (frame, bytes) in overrides {
            assert_eq!(bytes.len(), 256);
            data[frame * 256..frame * 256 + 256].copy_from_slice(bytes);
        }
        SpriteSet { width: 16, height: 16, count, data }
    }

    #[test]
    fn blit_stone_overwrites_nonzero_texels_and_clips_to_full_height() {
        // texel (x_, y_) = 100 + y_*16 + x_, except texel (1, 0) = 0 (transparent).
        let mut spr: Vec<u8> = (0..256).map(|i| 100u8.wrapping_add(i as u8)).collect();
        spr[1] = 0;
        let sprites = bank(4, &[(3, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        level.material_id.iter_mut().for_each(|p| *p = 7);
        blit_stone(&mut level, &sprites, 3, 15, 5); // clipped to x 15..20, y 5..10
        let at = |x: i32, y: i32| level.material_id[(x + y * 20) as usize];
        assert_eq!(at(15, 5), 100, "texel (0,0)");
        assert_eq!(at(16, 5), 7, "zero texel is transparent");
        assert_eq!(at(19, 9), 100 + 4 * 16 + 4, "bottom row IS written: clip is Rect(0,0,w,h)");
        assert_eq!(at(14, 5), 7, "left of the window untouched");
    }

    #[test]
    fn blit_stone_negative_origin_offsets_the_source_like_clip_image() {
        let spr: Vec<u8> = (0..256).map(|i| 1u8.wrapping_add(i as u8)).collect();
        let sprites = bank(1, &[(0, spr)]);
        let mut level = new_level(20, 10, &[0u8; 256]);
        blit_stone(&mut level, &sprites, 0, -3, -2); // visible window: x 0..13, y 0..14∩0..10
        assert_eq!(level.material_id[0], 1 + 2 * 16 + 3, "(0,0) <- texel (3,2)");
        assert_eq!(level.material_id[12], 1 + 2 * 16 + 15, "(12,0) <- texel (15,2)");
        assert_eq!(level.material_id[13], 0, "(13,0) is outside the clipped window");
    }

    #[test]
    fn splat_blends_177_to_179_and_overwrites_everything_else() {
        // All-150 image except texel (5,0) = 0; row 0 destination = 176,177,178,179,180,55.
        let mut img = vec![150u8; 256];
        img[5] = 0;
        let mut level = new_level(16, 16, &[0u8; 256]);
        for (x, v) in [176u8, 177, 178, 179, 180, 55].iter().enumerate() {
            level.material_id[x] = *v;
        }
        splat_sprite(&mut level, &img, 0, 0);
        // (150+177)/2 = 163, (150+178)/2 = 164, (150+179)/2 = 164 (level.cpp:63-64).
        assert_eq!(&level.material_id[0..6], &[150, 163, 164, 164, 150, 55]);
        assert_eq!(level.material_id[16], 150, "(0,1) overwritten");
    }

    #[test]
    fn splat_clips_with_break_and_continue_at_every_edge() {
        let img = vec![9u8; 256];
        let mut a = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut a, &img, -8, -8); // rand(w)-8 can be -8: cells 0..8 x 0..8, h=6
        let at = |l: &LevelSim, x: i32, y: i32| l.material_id[(x + y * 10) as usize];
        assert_eq!(at(&a, 0, 0), 9);
        assert_eq!(at(&a, 7, 5), 9);
        assert_eq!(at(&a, 8, 5), 0, "x = kX + 16 is past the sprite");
        let mut b = new_level(10, 6, &[0u8; 256]);
        splat_sprite(&mut b, &img, 5, 3); // breaks at x = 10 and y = 6
        assert_eq!(at(&b, 9, 5), 9);
        assert_eq!(at(&b, 4, 3), 0);
        assert_eq!(at(&b, 5, 2), 0);
    }

    #[test]
    fn splats_draw_one_count_plus_three_per_splat() {
        let sprites = bank(73, &[]); // frames 69..=72 must exist
        let mut level = new_level(40, 30, &[0u8; 256]);
        let mut rand = seeded(5);
        splat_large_sprites(&mut level, &sprites, &mut rand);
        let mut r = seeded(5);
        let count = r.bound(100) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
    }

    #[test]
    fn stones_draw_one_count_plus_three_per_stone_from_frames_56_to_59() {
        let solid = |v: u8| vec![v; 256];
        let sprites = bank(60, &[(56, solid(56)), (57, solid(57)), (58, solid(58)), (59, solid(59))]);
        let mut level = new_level(64, 48, &[0u8; 256]);
        let mut rand = seeded(11);
        scatter_stones(&mut level, &sprites, &mut rand);
        let mut r = seeded(11);
        let count = r.bound(15) as u64;
        assert_eq!(rand.draws(), 1 + 3 * count);
        assert!(level.material_id.iter().all(|&p| p == 0 || (56..=59).contains(&p)));
    }

    #[test]
    fn dirt_pattern_is_field_then_splats_then_stones() {
        let sprites = bank(73, &[(70, vec![200; 256]), (57, vec![30; 256])]);
        let mut a = new_level(50, 40, &[0u8; 256]);
        let mut ra = seeded(3);
        generate_dirt_pattern(&mut a, &sprites, &mut ra);
        let mut b = new_level(50, 40, &[0u8; 256]);
        let mut rb = seeded(3);
        generate_dirt_field(&mut b, &mut rb);
        splat_large_sprites(&mut b, &sprites, &mut rb);
        scatter_stones(&mut b, &sprites, &mut rb);
        assert_eq!(a, b);
        assert_eq!(ra.last(), rb.last());
    }
```

Add to the imports at the top of `rust/sim/src/levelgen.rs` (below `use sim_core::rng::Rand;`, keeping `std`/external/crate grouping):

```rust
use assets::sprite::SpriteSet;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile errors `cannot find function `blit_stone``, `splat_sprite`, `splat_large_sprites`, `scatter_stones`, `generate_dirt_pattern`.

- [ ] **Step 3: Implement**

Insert after `generate_dirt_field` in `rust/sim/src/levelgen.rs`:

```rust
/// Port of `BlitStone(common, level, p1 = false, mem, x, y)` (`gfx/blit.cpp:462-532`).
/// `CLIP_IMAGE` (`gfx/macros.hpp:3-22`) against **`Rect(0, 0, width, height)` — the full
/// height** (unlike `DrawDirtEffect`'s `height - 1`), then every NON-ZERO texel overwrites
/// the destination (the `p1 = false` branch has no material test, `:505-531`). Every
/// C++ caller is the generator and passes `p1 = false`; the `p1 = true` branch has no caller
/// and is not ported.
pub fn blit_stone(level: &mut LevelSim, large_sprites: &SpriteSet, frame: usize, x: i32, y: i32) {
    let sprite = large_sprites.sprite(frame);
    let pitch: i32 = 16;
    let mut w: i32 = 16;
    let mut h: i32 = 16;
    let mut mem: i32 = 0;
    let mut x = x;
    let mut y = y;
    let (cx1, cy1, cx2, cy2) = (0i32, 0i32, level.width, level.height);
    let top = y - cy1;
    if top < 0 {
        mem += -top * pitch;
        h += top;
        y = cy1;
    }
    let bottom = y + h - cy2;
    if bottom > 0 {
        h -= bottom;
    }
    let left = x - cx1;
    if left < 0 {
        mem -= left;
        w += left;
        x = cx1;
    }
    let right = x + w - cx2;
    if right > 0 {
        w -= right;
    }
    if w <= 0 || h <= 0 {
        return;
    }
    let level_width = level.width;
    for y_ in 0..h {
        for x_ in 0..w {
            let c = sprite[(mem + y_ * pitch + x_) as usize];
            if c != 0 {
                level.set_material(((y + y_) * level_width + x + x_) as usize, c);
            }
        }
    }
}

/// One large-sprite splat (`level.cpp:38-70`): rows `my >= height` BREAK, `my < 0`
/// CONTINUE (same for columns) — a literal port, not a clamp. A non-zero texel over a
/// destination in 177..=179 (`> 176 && < 180`) blends to `(src + dest) / 2`, otherwise it
/// overwrites.
fn splat_sprite(level: &mut LevelSim, image: &[u8], kx: i32, ky: i32) {
    for cy in 0..16 {
        let my = cy + ky;
        if my >= level.height {
            break;
        }
        if my < 0 {
            continue;
        }
        for cx in 0..16 {
            let mx = cx + kx;
            if mx >= level.width {
                break;
            }
            if mx < 0 {
                continue;
            }
            let src = image[((cy << 4) + cx) as usize];
            if src > 0 {
                let idx = (mx + my * level.width) as usize;
                let pix = level.material_id[idx];
                let v = if pix > 176 && pix < 180 {
                    ((src as u32 + pix as u32) / 2) as u8
                } else {
                    src
                };
                level.set_material(idx, v);
            }
        }
    }
}

/// `GenerateDirtPattern`'s splat block (`level.cpp:30-71`): `count = rand(100)`, then per
/// splat `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+69`. Draws `1 + 3*count`.
pub fn splat_large_sprites(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(100) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 69;
        splat_sprite(level, large_sprites.sprite(frame), kx, ky);
    }
}

/// `GenerateDirtPattern`'s stone block (`level.cpp:73-82`): `count = rand(15)`, then per
/// stone `x = rand(w)-8`, `y = rand(h)-8`, frame `rand(4)+56`, [`blit_stone`]. Draws
/// `1 + 3*count`.
pub fn scatter_stones(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    let count = rand.bound(15) as i32;
    for _ in 0..count {
        let kx = rand.bound(level.width as u32) as i32 - 8;
        let ky = rand.bound(level.height as u32) as i32 - 8;
        let frame = rand.bound(4) as usize + 56;
        blit_stone(level, large_sprites, frame, kx, ky);
    }
}

/// `Level::GenerateDirtPattern` (`level.cpp:11-83`) = field, splats, stones.
pub fn generate_dirt_pattern(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) {
    generate_dirt_field(level, rand);
    splat_large_sprites(level, large_sprites, rand);
    scatter_stones(level, large_sprites, rand);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 10 passed; 0 failed`.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (else run without `--check`).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): BlitStone + GenerateDirtPattern splats/stones" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

### Task 3 [Opus]: worm tunnels over the Step-2 `draw_dirt_effect`

**Files:**
- Modify: `rust/sim/src/levelgen.rs` (imports; `tunnel_walk` + `dig_tunnels` after `generate_dirt_pattern`; tests appended inside `mod tests`)

**Interfaces:**
- Consumes: T1's `new_level`; T2's test helper `bank`; `sim::blit::draw_dirt_effect(level: &mut LevelSim, large_sprites: &SpriteSet, textures: &[Texture], dirt_effect: i32, x: i32, y: i32, rand: &mut Rand)` (`rust/sim/src/blit.rs:42-50` — draws `rand(r_frame)` BEFORE clipping to `Rect(0,0,w,h-1)`); `assets::tc::Texture { mframe: i32, rframe: i32, sframe: i32, ndrawback: bool }` (`assets/src/tc.rs:145-150`, derives `Clone`); `sim::state::{MAT_DIRT, MAT_BACKGROUND}`.
- Produces:
  - `pub fn dig_tunnels(level: &mut LevelSim, large_sprites: &SpriteSet, textures: &[Texture], rand: &mut Rand)`
  - private `fn tunnel_walk(width: i32, height: i32, rand: &mut Rand, stamp: impl FnMut(&mut Rand, i32, i32))`

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `rust/sim/src/levelgen.rs`:

```rust
    /// level.cpp:108-135 restated statement by statement (the stamp recorder draws one
    /// value per stamp, as DrawDirtEffect's rand(r_frame) does). Pins the draw order, the
    /// `(count3 + 1) * d` backtrack and the jitter ranges in readable form; the golden's
    /// `tunnels` stage token is the correctness gate.
    #[test]
    fn tunnel_walk_follows_level_cpp_108_135() {
        let (w, h) = (300, 200);
        let mut got = Vec::new();
        let mut rand = seeded(99);
        tunnel_walk(w, h, &mut rand, |r, x, y| {
            r.bound(2);
            got.push((x, y));
        });

        let mut r = seeded(99);
        let mut want = Vec::new();
        let count = r.bound(50) as i32 + 5;
        for _ in 0..count {
            let mut cx = r.bound(w as u32) as i32 - 8;
            let mut cy = r.bound(h as u32) as i32 - 8;
            let dx = r.bound(11) as i32 - 5;
            let dy = r.bound(5) as i32 - 2;
            let count2 = r.bound(12) as i32;
            for _ in 0..count2 {
                let count3 = r.bound(5) as i32;
                for _ in 0..count3 {
                    cx += dx;
                    cy += dy;
                    r.bound(2);
                    want.push((cx, cy));
                }
                cx -= (count3 + 1) * dx;
                cy -= (count3 + 1) * dy;
                cx += r.bound(7) as i32 - 3;
                cy += r.bound(15) as i32 - 7;
            }
        }
        assert_eq!(got, want);
        assert_eq!(rand.draws(), r.draws());
        assert!(!got.is_empty());
    }

    /// Every stamp draws rand(r_frame) even when it is fully clipped (blit.cpp:537 runs
    /// before the clip at :545-547). On a 4x4 level almost every 16x16 stamp is clipped, so
    /// a port that skipped the draw for off-level stamps would diverge here.
    #[test]
    fn dig_tunnels_draws_once_per_stamp_even_when_clipped() {
        let tex = Texture { mframe: 0, rframe: 2, sframe: 1, ndrawback: true };
        let textures = vec![tex.clone(), tex];
        let sprites = bank(3, &[]);
        let mut level = new_level(4, 4, &[0u8; 256]);
        let mut rand = seeded(21);
        dig_tunnels(&mut level, &sprites, &textures, &mut rand);

        let mut r = seeded(21);
        let mut stamps = 0u64;
        tunnel_walk(4, 4, &mut r, |r, _, _| {
            r.bound(2);
            stamps += 1;
        });
        assert!(stamps > 0);
        assert_eq!(rand.draws(), r.draws());
        assert_eq!(rand.last(), r.last());
    }

    /// dirt effect 1 = tc.cfg texture 1 (mframe 1, carving: ndrawback = true). With an
    /// all-1 mask over all-Dirt terrain every in-clip window cell becomes material 1
    /// (blit.cpp:566-579), the window's top-left is the walked (cx, cy) — no -7 offset —
    /// and the clip is Rect(0, 0, w, h - 1).
    #[test]
    fn dig_tunnels_carves_texture_1_at_the_walked_top_left_positions() {
        let mut flags = [0u8; 256];
        flags[12] = MAT_DIRT;
        flags[1] = MAT_DIRT | MAT_BACKGROUND; // tc.cfg materials[1] = 9
        let textures = vec![
            Texture { mframe: 0, rframe: 2, sframe: 2, ndrawback: true },
            Texture { mframe: 1, rframe: 2, sframe: 2, ndrawback: true },
        ];
        let sprites = bank(4, &[(1, vec![1u8; 256])]);
        let (w, h) = (64, 48);
        let mut level = new_level(w, h, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 12);
        let mut rand = seeded(8);
        dig_tunnels(&mut level, &sprites, &textures, &mut rand);

        let mut r = seeded(8);
        let mut want = vec![12u8; (w * h) as usize];
        tunnel_walk(w, h, &mut r, |r, x, y| {
            r.bound(2);
            for my in y.max(0)..(y + 16).min(h - 1) {
                for mx in x.max(0)..(x + 16).min(w) {
                    want[(mx + my * w) as usize] = 1;
                }
            }
        });
        assert_eq!(level.material_id, want);
    }
```

Add to the test-module imports (below `use crate::state::MAT_ROCK;`):

```rust
    use crate::state::{MAT_BACKGROUND, MAT_DIRT};
```

Add to the top-of-file imports of `rust/sim/src/levelgen.rs`:

```rust
use assets::tc::Texture;

use crate::blit::draw_dirt_effect;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile errors `cannot find function `tunnel_walk`` and `dig_tunnels`.

- [ ] **Step 3: Implement**

Insert after `generate_dirt_pattern` in `rust/sim/src/levelgen.rs`:

```rust
/// The tunnel walk of `GenerateRandom` (`level.cpp:108-135`), with the per-stamp action
/// injected so tests can record positions. `count = rand(50)+5` tunnels; each starts at
/// `(rand(w)-8, rand(h)-8)` with step `(rand(11)-5, rand(5)-2)` and `rand(12)` segments; a
/// segment draws `count3 = rand(5)`, takes `count3` steps (stamping after each), backtracks
/// by `(count3 + 1) * step`, then jitters by `(rand(7)-3, rand(15)-7)`.
fn tunnel_walk(width: i32, height: i32, rand: &mut Rand, mut stamp: impl FnMut(&mut Rand, i32, i32)) {
    let count = rand.bound(50) as i32 + 5;
    for _ in 0..count {
        let mut cx = rand.bound(width as u32) as i32 - 8;
        let mut cy = rand.bound(height as u32) as i32 - 8;
        let dx = rand.bound(11) as i32 - 5;
        let dy = rand.bound(5) as i32 - 2;
        let count2 = rand.bound(12) as i32;
        for _ in 0..count2 {
            let count3 = rand.bound(5) as i32;
            for _ in 0..count3 {
                cx += dx;
                cy += dy;
                stamp(&mut *rand, cx, cy);
            }
            cx -= (count3 + 1) * dx;
            cy -= (count3 + 1) * dy;
            cx += rand.bound(7) as i32 - 3;
            cy += rand.bound(15) as i32 - 7;
        }
    }
}

/// `GenerateRandom`'s dirt-effect worm tunnels (`level.cpp:108-135`): every stamp is
/// `DrawDirtEffect(texture 1, cx, cy)` with `(cx, cy)` the window's TOP-LEFT (no `-7`,
/// unlike `BlowUpObject`). Texture 1 is the carving texture (tc.cfg `mframe=1, rframe=2,
/// sframe=73, ndrawback=true`). Each stamp draws its own `rand(r_frame)` first, even when
/// it is clipped away entirely (`blit.cpp:537`) — reused unchanged from Step 2
/// ([`draw_dirt_effect`]).
pub fn dig_tunnels(level: &mut LevelSim, large_sprites: &SpriteSet, textures: &[Texture], rand: &mut Rand) {
    let (w, h) = (level.width, level.height);
    tunnel_walk(w, h, rand, |rand, x, y| {
        draw_dirt_effect(level, large_sprites, textures, 1, x, y, rand)
    });
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 13 passed; 0 failed`.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (else run without `--check`).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): GenerateRandom worm tunnels via draw_dirt_effect" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

### Task 4 [Opus]: rock formations, rocks, the retry cap, `generate_random`

**Files:**
- Modify: `rust/sim/src/levelgen.rs` (`STONE_TAB`, `RockStats`, `LevelGenAssets` after `MAX_DIM`; `is_no_rock`, `blit_formation`, `place_rock_formations`, `place_rocks`, `generate_random` after `dig_tunnels`; tests appended inside `mod tests`)

**Interfaces:**
- Consumes: T1 `new_level`; T2 `blit_stone`, `generate_dirt_pattern`, test helper `bank`; T3 `dig_tunnels`; `LevelSim::rock(&self, x: i32, y: i32) -> bool` (`state.rs:791-794`, in-bounds only); `assets::tc::TcConfig::load(&[u8]) -> Result<TcConfig, TcError>` (`.materials: [u8; 256]`, `.textures: Vec<Texture>`), `assets::sprite::Tga::load`, `SpriteSet::from_tga(&tga, 16, 16, 110)`.
- Produces:
  - `pub const STONE_TAB: [[usize; 4]; 3]`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct RockStats { pub count: i32, pub placed: i32, pub tries: u64 }`
  - `pub struct LevelGenAssets<'a> { pub large_sprites: &'a SpriteSet, pub textures: &'a [Texture], pub material_flags: &'a [u8; 256] }`
  - `pub fn blit_formation(level: &mut LevelSim, large_sprites: &SpriteSet, kind: usize, cx: i32, cy: i32)`
  - `pub fn place_rock_formations(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) -> RockStats`
  - `pub fn place_rocks(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) -> RockStats`
  - `pub fn generate_random(assets: &LevelGenAssets, width: i32, height: i32, rand: &mut Rand) -> LevelSim`
  - private `fn is_no_rock(level: &LevelSim, size: i32, x: i32, y: i32) -> bool`
  - test helper `fn real_tc() -> (SpriteSet, Vec<Texture>, [u8; 256])` (used by T6)

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `rust/sim/src/levelgen.rs`:

```rust
    /// The shipped TC's large-sprite bank, texture table and material flags.
    fn real_tc() -> (SpriteSet, Vec<Texture>, [u8; 256]) {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/TC/openliero");
        let tc_bytes = std::fs::read(format!("{root}/tc.cfg")).expect("read tc.cfg");
        let tc = assets::tc::TcConfig::load(&tc_bytes).expect("tc.cfg parses");
        let tga_bytes = std::fs::read(format!("{root}/sprites/large.tga")).expect("read large.tga");
        let tga = assets::sprite::Tga::load(&tga_bytes).expect("large.tga parses");
        let large = SpriteSet::from_tga(&tga, 16, 16, 110).expect("large sprite bank");
        (large, tc.textures.clone(), tc.materials)
    }

    #[test]
    fn is_no_rock_checks_a_size_plus_one_window_clipped_to_the_level() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(40, 40, &flags);
        level.material_id[(10 + 10 * 40) as usize] = 5; // rock at (10,10)
        assert!(!is_no_rock(&level, 4, 6, 6), "(10,10) is the far corner of the 5x5 window");
        assert!(is_no_rock(&level, 4, 5, 5), "window 5..=9 excludes (10,10)");
        assert!(!is_no_rock(&level, 15, -5, -5), "negative origin clips to 0..11");
        assert!(is_no_rock(&level, 32, 39, 39), "window clipped to the single cell (39,39)");
    }

    #[test]
    fn blit_formation_lays_stone_tab_out_as_2x2() {
        let overrides: Vec<(usize, Vec<u8>)> =
            STONE_TAB[1].iter().map(|&f| (f, vec![f as u8; 256])).collect();
        let sprites = bank(99, &overrides);
        let mut level = new_level(40, 40, &[0u8; 256]);
        blit_formation(&mut level, &sprites, 1, 4, 2);
        let at = |x: i32, y: i32| level.material_id[(x + y * 40) as usize];
        assert_eq!(at(4, 2), 63, "top-left = stone_tab[1][0]");
        assert_eq!(at(20, 2), 75, "top-right = stone_tab[1][1] at cx+16");
        assert_eq!(at(4, 18), 85, "bottom-left = stone_tab[1][2] at cy+16");
        assert_eq!(at(20, 18), 86, "bottom-right = stone_tab[1][3]");
    }

    /// 8x8 all-rock: every candidate is rejected, kMaxTries = 64 per formation, and on the
    /// cap the loop `continue`s BEFORE rand(3) (level.cpp:155-160).
    #[test]
    fn formations_on_solid_rock_hit_the_cap_and_skip_the_kind_draw() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(8, 8, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 5);
        let sprites = bank(99, &[]);
        let mut rand = seeded(13);
        let stats = place_rock_formations(&mut level, &sprites, &mut rand);

        let mut r = seeded(13);
        let count = r.bound(15) as i32 + 5;
        for _ in 0..count {
            for _ in 0..64 {
                r.bound(8);
                if r.bound(4) == 0 {
                    r.bound(20);
                } else {
                    r.bound(8);
                }
            }
        }
        assert_eq!(stats, RockStats { count, placed: 0, tries: 64 * count as u64 });
        assert_eq!(rand.draws(), r.draws(), "no rand(3) after a capped formation");
        assert!(level.material_id.iter().all(|&p| p == 5), "nothing blitted");
    }

    /// Same for the 16x16 rocks (level.cpp:172-192): candidates rand(w)-8 and
    /// rand(5)==0 ? h-1-rand(13) : rand(h)-8; on the cap no rand(6).
    #[test]
    fn rocks_on_solid_rock_hit_the_cap_and_skip_the_sprite_draw() {
        let mut flags = [0u8; 256];
        flags[5] = MAT_ROCK;
        let mut level = new_level(8, 8, &flags);
        level.material_id.iter_mut().for_each(|p| *p = 5);
        let sprites = bank(99, &[]);
        let mut rand = seeded(14);
        let stats = place_rocks(&mut level, &sprites, &mut rand);

        let mut r = seeded(14);
        let count = r.bound(25) as i32 + 5;
        for _ in 0..count {
            for _ in 0..64 {
                r.bound(8);
                if r.bound(5) == 0 {
                    r.bound(13);
                } else {
                    r.bound(8);
                }
            }
        }
        assert_eq!(stats, RockStats { count, placed: 0, tries: 64 * count as u64 });
        assert_eq!(rand.draws(), r.draws(), "no rand(6) after a capped rock");
    }

    /// No rock anywhere and all-zero sprites (so blits write nothing): every formation and
    /// every rock is accepted on its first candidate.
    #[test]
    fn open_terrain_accepts_every_first_candidate() {
        let mut level = new_level(200, 150, &[0u8; 256]);
        let sprites = bank(99, &[]);
        let mut rand = seeded(17);
        let f = place_rock_formations(&mut level, &sprites, &mut rand);
        assert!((5..=19).contains(&f.count));
        assert_eq!((f.placed, f.tries), (f.count, f.count as u64));
        let k = place_rocks(&mut level, &sprites, &mut rand);
        assert!((5..=29).contains(&k.count));
        assert_eq!((k.placed, k.tries), (k.count, k.count as u64));
    }

    #[test]
    fn generate_random_composes_the_stages_in_level_cpp_order() {
        let (large, textures, flags) = real_tc();
        let assets = LevelGenAssets { large_sprites: &large, textures: &textures, material_flags: &flags };
        let mut ra = seeded(42);
        let a = generate_random(&assets, 504, 350, &mut ra);

        let mut rb = seeded(42);
        let mut b = new_level(504, 350, &flags);
        generate_dirt_pattern(&mut b, &large, &mut rb);
        dig_tunnels(&mut b, &large, &textures, &mut rb);
        place_rock_formations(&mut b, &large, &mut rb);
        place_rocks(&mut b, &large, &mut rb);
        assert_eq!(a, b);
        assert_eq!(ra.last(), rb.last());
        assert!(a.material_id.iter().any(|&m| flags[m as usize] & MAT_ROCK != 0), "rocks placed");
        assert!(
            a.material_id.iter().any(|&m| flags[m as usize] & MAT_BACKGROUND != 0),
            "tunnels carved background (tc.cfg materials 1/2 carry the background bit)"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile errors `cannot find function `is_no_rock``, `blit_formation`, `place_rock_formations`, `place_rocks`, `generate_random`, and `cannot find value `STONE_TAB``, `cannot find struct `RockStats``, `LevelGenAssets`.

- [ ] **Step 3: Implement the types**

Insert after `const MAX_DIM` in `rust/sim/src/levelgen.rs`:

```rust
/// `stone_tab` (`common.cpp:23`): the four large-sprite frames of each 32x32 rock
/// formation, in quadrant order top-left, top-right, bottom-left, bottom-right.
pub const STONE_TAB: [[usize; 4]; 3] = [[98, 60, 61, 62], [63, 75, 85, 86], [89, 90, 97, 96]];

/// Diagnostic statistics of one rock-placement loop (no C++ counterpart — C++ returns
/// nothing). `count` = the drawn number of items, `placed` = how many found a rock-free
/// window before the `kMaxTries` cap, `tries` = candidate positions drawn (loop
/// iterations) summed over all items. The golden compares all three, which proves the cap
/// path and the retry path ran and localises a divergence to the predicate vs the draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RockStats {
    pub count: i32,
    pub placed: i32,
    pub tries: u64,
}

/// The TC assets generation reads (C++ `Common`): the 16x16 large-sprite bank (110
/// frames, `common.cpp:368`), the texture table (tc.cfg `[[constants.textures]]`) and the
/// 256-entry material flag table (tc.cfg `materials`).
pub struct LevelGenAssets<'a> {
    pub large_sprites: &'a SpriteSet,
    pub textures: &'a [Texture],
    pub material_flags: &'a [u8; 256],
}
```

- [ ] **Step 4: Implement the loops and the composition**

Insert after `dig_tunnels` in `rust/sim/src/levelgen.rs`:

```rust
/// Port of the file-static `IsNoRock` (`level.cpp:85-99`): the `(size+1) x (size+1)`
/// window at `(x, y)`, intersected with the level, contains no `Rock()` cell. A window
/// clipped to nothing is rock-free.
fn is_no_rock(level: &LevelSim, size: i32, x: i32, y: i32) -> bool {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + size + 1).min(level.width);
    let y2 = (y + size + 1).min(level.height);
    for yy in y1..y2 {
        for xx in x1..x2 {
            if level.rock(xx, yy) {
                return false;
            }
        }
    }
    true
}

/// The 2x2 [`blit_stone`] of one 32x32 formation (`level.cpp:162-169`).
pub fn blit_formation(level: &mut LevelSim, large_sprites: &SpriteSet, kind: usize, cx: i32, cy: i32) {
    let t = STONE_TAB[kind];
    blit_stone(level, large_sprites, t[0], cx, cy);
    blit_stone(level, large_sprites, t[1], cx + 16, cy);
    blit_stone(level, large_sprites, t[2], cx, cy + 16);
    blit_stone(level, large_sprites, t[3], cx + 16, cy + 16);
}

/// `GenerateRandom`'s rock formations (`level.cpp:137-170`). `kMaxTries = w*h`;
/// `count = rand(15)+5`; per formation draw candidates `cx = rand(w)-16`,
/// `cy = rand(4)==0 ? h-1-rand(20) : rand(h)-16` until `is_no_rock(32)` accepts or the
/// rejection counter (incremented ONLY on rejection) reaches the cap; on the cap skip the
/// formation WITHOUT drawing its kind; else `kind = rand(3)` and [`blit_formation`].
pub fn place_rock_formations(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) -> RockStats {
    let max_tries = level.width * level.height;
    let mut s = RockStats { count: rand.bound(15) as i32 + 5, placed: 0, tries: 0 };
    for _ in 0..s.count {
        let mut tries = 0i32;
        let mut cx: i32;
        let mut cy: i32;
        loop {
            s.tries += 1;
            cx = rand.bound(level.width as u32) as i32 - 16;
            cy = if rand.bound(4) == 0 {
                level.height - 1 - rand.bound(20) as i32
            } else {
                rand.bound(level.height as u32) as i32 - 16
            };
            if is_no_rock(level, 32, cx, cy) {
                break;
            }
            tries += 1;
            if tries >= max_tries {
                break;
            }
        }
        if tries >= max_tries {
            continue;
        }
        s.placed += 1;
        let kind = rand.bound(3) as usize;
        blit_formation(level, large_sprites, kind, cx, cy);
    }
    s
}

/// `GenerateRandom`'s 16x16 rocks (`level.cpp:172-192`): `count = rand(25)+5`; candidates
/// `cx = rand(w)-8`, `cy = rand(5)==0 ? h-1-rand(13) : rand(h)-8`; `is_no_rock(15)`;
/// same cap rule; placed ⇒ frame `rand(6)+3` via [`blit_stone`].
pub fn place_rocks(level: &mut LevelSim, large_sprites: &SpriteSet, rand: &mut Rand) -> RockStats {
    let max_tries = level.width * level.height;
    let mut s = RockStats { count: rand.bound(25) as i32 + 5, placed: 0, tries: 0 };
    for _ in 0..s.count {
        let mut tries = 0i32;
        let mut cx: i32;
        let mut cy: i32;
        loop {
            s.tries += 1;
            cx = rand.bound(level.width as u32) as i32 - 8;
            cy = if rand.bound(5) == 0 {
                level.height - 1 - rand.bound(13) as i32
            } else {
                rand.bound(level.height as u32) as i32 - 8
            };
            if is_no_rock(level, 15, cx, cy) {
                break;
            }
            tries += 1;
            if tries >= max_tries {
                break;
            }
        }
        if tries >= max_tries {
            continue;
        }
        s.placed += 1;
        let frame = rand.bound(6) as usize + 3;
        blit_stone(level, large_sprites, frame, cx, cy);
    }
    s
}

/// `Level::GenerateRandom` (`level.cpp:101-193`) minus the palette reset (a generated level
/// has no custom palette — [`generate_from_settings`] returns `palette: None`, design F8).
/// Stages in C++ order: resize, dirt pattern, tunnels, formations, rocks.
pub fn generate_random(assets: &LevelGenAssets, width: i32, height: i32, rand: &mut Rand) -> LevelSim {
    let mut level = new_level(width, height, assets.material_flags);
    generate_dirt_pattern(&mut level, assets.large_sprites, rand);
    dig_tunnels(&mut level, assets.large_sprites, assets.textures, rand);
    place_rock_formations(&mut level, assets.large_sprites, rand);
    place_rocks(&mut level, assets.large_sprites, rand);
    level
}
```

(The doc link to `generate_from_settings` resolves once T6 lands; until then rustdoc only warns.)

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 19 passed; 0 failed`.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (else run without `--check`).

- [ ] **Step 6: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): rock formations + rocks with IsNoRock and the kMaxTries cap; generate_random" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

### Task 5 [Opus]: `make_shadow` — the whole-level `MakeShadow` pass

`MakeShadow` lives in `levelgen.rs` (not a `shadow` module): the parallel slice 4½a creates `rust/sim/src/shadow.rs` for `CorrectShadow` (4½a design §5.1). 4½b never ports `CorrectShadow`; T9 checks 4½a's port against this slice's golden.

**Files:**
- Modify: `rust/sim/src/levelgen.rs` (imports; `MAT_SEE_SHADOW` after `MAX_DIM`; `flags_at` + `make_shadow` after `generate_random`; tests appended inside `mod tests`)

**Interfaces:**
- Consumes: T1 `new_level` (the test helper `shadow_level` builds on it); T3's test imports `MAT_DIRT`, `MAT_BACKGROUND`; `sim::state::{LevelSim, MAT_BACKGROUND, MAT_DIRT, MAT_DIRT_ROCK, MAT_ROCK}` (`state.rs:640-663`), `LevelSim::set_material`.
- Produces:
  - `pub fn make_shadow(level: &mut LevelSim)`
  - private `const MAT_SEE_SHADOW: u8 = 1 << 4` (`material.hpp:11`) and `fn flags_at(level: &LevelSim, x: i32, y: i32) -> u8`

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `rust/sim/src/levelgen.rs`:

```rust
    /// Flags shaped like the shipped tc.cfg `materials`: dirt 12..=18 (1), rock 19 (4),
    /// see-shadow background 160..=163 (24), their shadowed twins 164..=167 (8), and
    /// dirt+background 1 (9). Material 40 has no flags (the default fill).
    fn shadow_flags() -> [u8; 256] {
        let mut f = [0u8; 256];
        for m in 12..=18 {
            f[m] = MAT_DIRT;
        }
        f[19] = MAT_ROCK;
        for m in 160..=163 {
            f[m] = MAT_SEE_SHADOW | MAT_BACKGROUND;
        }
        for m in 164..=167 {
            f[m] = MAT_BACKGROUND;
        }
        f[1] = MAT_DIRT | MAT_BACKGROUND;
        f
    }

    fn shadow_level(w: i32, h: i32) -> LevelSim {
        let mut l = new_level(w, h, &shadow_flags());
        l.material_id.iter_mut().for_each(|p| *p = 40);
        l
    }

    fn set(l: &mut LevelSim, x: i32, y: i32, v: u8) {
        let w = l.width;
        l.material_id[(x + y * w) as usize] = v;
    }

    fn get(l: &LevelSim, x: i32, y: i32) -> u8 {
        l.material_id[(x + y * l.width) as usize]
    }

    #[test]
    fn make_shadow_adds_4_to_see_shadow_pixels_under_dirt_rock() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 2, 5, 160);
        set(&mut l, 5, 2, 12); // DirtRock neighbour of (2,5)
        set(&mut l, 2, 7, 161); // neighbour (5,4) = 40: no flags
        make_shadow(&mut l);
        assert_eq!(get(&l, 2, 5), 164);
        assert_eq!(get(&l, 2, 7), 161);
    }

    #[test]
    fn make_shadow_dims_dirt_beside_rock_floored_at_12_and_skips_the_last_3_columns() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 1, 6, 16);
        set(&mut l, 4, 3, 19); // rock neighbour of (1,6): 16 - 2 = 14
        set(&mut l, 1, 8, 13);
        set(&mut l, 4, 5, 19); // rock neighbour of (1,8): 13 - 2 = 11 -> floor 12
        set(&mut l, 6, 6, 14);
        set(&mut l, 9, 3, 19); // x = 6 < w - 3: processed -> 12
        set(&mut l, 7, 6, 14); // x = 7 = w - 3: never processed
        make_shadow(&mut l);
        assert_eq!(get(&l, 1, 6), 14);
        assert_eq!(get(&l, 1, 8), 12);
        assert_eq!(get(&l, 6, 6), 12);
        assert_eq!(get(&l, 7, 6), 14);
    }

    /// Rule 2 re-reads the pixel AFTER rule 1 wrote it (level.cpp:198-206): a see-shadow
    /// 10 becomes 14, which is in 12..=18 with a rock neighbour, so it drops to 12.
    #[test]
    fn make_shadow_rule_two_rereads_the_rule_one_result() {
        let mut l = shadow_level(10, 10);
        l.material_flags[10] = MAT_SEE_SHADOW;
        set(&mut l, 3, 6, 10);
        set(&mut l, 6, 3, 19); // rock: DirtRock for rule 1 AND Rock for rule 2
        make_shadow(&mut l);
        assert_eq!(get(&l, 3, 6), 12);
    }

    /// A = (0,6) looks at B = (3,3); B looks at C = (6,0). B turns 161 -> 165 and 165 is
    /// made DirtRock here, so if B were processed before A (a y-outer loop), A would darken.
    #[test]
    fn make_shadow_reads_neighbours_before_their_column_is_visited() {
        let mut l = shadow_level(10, 10);
        l.material_flags[165] = MAT_DIRT | MAT_BACKGROUND;
        set(&mut l, 0, 6, 160);
        set(&mut l, 3, 3, 161);
        set(&mut l, 6, 0, 12);
        make_shadow(&mut l);
        assert_eq!(get(&l, 3, 3), 165, "B shadowed by C");
        assert_eq!(get(&l, 0, 6), 160, "A read B before B changed");
    }

    #[test]
    fn make_shadow_sets_background_bottom_row_pixels_to_13() {
        let mut l = shadow_level(10, 10);
        set(&mut l, 0, 9, 1); // dirt+background -> 13
        set(&mut l, 9, 9, 164); // background, in the last 3 columns -> 13
        set(&mut l, 5, 9, 12); // dirt, not background -> unchanged
        make_shadow(&mut l);
        assert_eq!(get(&l, 0, 9), 13);
        assert_eq!(get(&l, 9, 9), 13);
        assert_eq!(get(&l, 5, 9), 12);
        assert_eq!(get(&l, 4, 9), 40);
    }

```

In the top-of-file imports of `rust/sim/src/levelgen.rs`, replace `use crate::state::LevelSim;` with

```rust
use crate::state::{LevelSim, MAT_BACKGROUND, MAT_DIRT_ROCK, MAT_ROCK};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile errors `cannot find value `MAT_SEE_SHADOW`` and `cannot find function `make_shadow``.

- [ ] **Step 3: Implement**

Insert after `const MAX_DIM` in `rust/sim/src/levelgen.rs`:

```rust
/// `Material::kSeeShadow` (`material.hpp:11`, `1 << 4`): a background shade that shows a
/// cast shadow. Same value as `render::shadow_query::MAT_SEE_SHADOW`; private here so 4½a's
/// `sim::shadow` stays the public home of the shadow flag (no duplicate `pub` item).
const MAT_SEE_SHADOW: u8 = 1 << 4;
```

Insert after `generate_random`:

```rust
/// Flag byte of the in-bounds pixel `(x, y)` — the C++ `Mat(x, y)` read, derived live
/// from `material_flags[material_id]`.
fn flags_at(level: &LevelSim, x: i32, y: i32) -> u8 {
    level.material_flags[level.material_id[(x + y * level.width) as usize] as usize]
}

/// Port of `Level::MakeShadow` (`level.cpp:195-216`). For `x in 0..w-3` (outer), `y in 3..h`
/// (inner): (1) `SeeShadow(x,y) && DirtRock(x+3,y-3)` ⇒ pixel `+4` (wrapping `PalIdx`);
/// (2) then, re-reading the possibly updated pixel, `12..=18 && Rock(x+3,y-3)` ⇒ `-2`,
/// floored at 12. Finally every `Background` pixel of the bottom row (all x) becomes 13.
/// Precondition `height >= 1`.
pub fn make_shadow(level: &mut LevelSim) {
    let w = level.width;
    let h = level.height;
    for x in 0..w - 3 {
        for y in 3..h {
            let idx = (x + y * w) as usize;
            if flags_at(level, x, y) & MAT_SEE_SHADOW != 0
                && flags_at(level, x + 3, y - 3) & MAT_DIRT_ROCK != 0
            {
                let p = level.material_id[idx];
                level.set_material(idx, p.wrapping_add(4));
            }
            let p = level.material_id[idx];
            if (12..=18).contains(&p) && flags_at(level, x + 3, y - 3) & MAT_ROCK != 0 {
                let dimmed = p - 2;
                level.set_material(idx, if dimmed < 12 { 12 } else { dimmed });
            }
        }
    }
    for x in 0..w {
        if flags_at(level, x, h - 1) & MAT_BACKGROUND != 0 {
            level.set_material((x + (h - 1) * w) as usize, 13);
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 24 passed; 0 failed`.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (else run without `--check`).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): MakeShadow whole-level pass" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

### Task 6 [Opus]: `generate_from_settings` — random / file / fallback + shadow

**Files:**
- Modify: `rust/sim/src/levelgen.rs` (imports; `LevelGenParams` after `LevelGenAssets`; `level_file_name` + `generate_from_settings` after `generate_random`; tests appended inside `mod tests`)

**Interfaces:**
- Consumes: T4 `generate_random`, `LevelGenAssets`, test helper `real_tc`; T5 `make_shadow` (same module); `assets::level::LevelData { width, height, material_id, palette: Option<Palette>, display: Option<DisplayLayers> }` (`assets/src/level.rs:8-16`); `assets::palette::{Color, Palette}` (`Color: Copy + Default`).
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct LevelGenParams { pub random_level: bool, pub random_map_width: i32, pub random_map_height: i32, pub shadow: bool }` + `impl Default` (C++ `Settings` defaults: `true, 504, 350, true`)
  - `pub fn level_file_name(level_file: &str) -> String`
  - `pub fn generate_from_settings(assets: &LevelGenAssets, params: &LevelGenParams, file: Option<LevelData>, rand: &mut Rand) -> LevelData`

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `rust/sim/src/levelgen.rs`:

```rust
    #[test]
    fn level_file_name_appends_lev_only_when_there_is_no_dot() {
        assert_eq!(level_file_name("Levels/foo"), "Levels/foo.LEV");
        assert_eq!(level_file_name("Levels/foo.lev"), "Levels/foo.lev");
        assert_eq!(level_file_name("a.b/c"), "a.b/c", "C++ searches the whole string for '.'");
    }

    #[test]
    fn level_gen_params_default_mirrors_cpp_settings() {
        let want = LevelGenParams { random_level: true, random_map_width: 504, random_map_height: 350, shadow: true };
        assert_eq!(LevelGenParams::default(), want);
    }

    #[test]
    fn random_path_generates_then_shadows_and_ignores_any_file() {
        let (large, textures, flags) = real_tc();
        let assets = LevelGenAssets { large_sprites: &large, textures: &textures, material_flags: &flags };
        let params = LevelGenParams { random_level: true, random_map_width: 128, random_map_height: 96, shadow: true };
        let decoy = LevelData { width: 2, height: 2, material_id: vec![1; 4], palette: None, display: None };
        let mut ra = seeded(9);
        let got = generate_from_settings(&assets, &params, Some(decoy), &mut ra);

        let mut rb = seeded(9);
        let mut want = generate_random(&assets, 128, 96, &mut rb);
        make_shadow(&mut want);
        assert_eq!((got.width, got.height), (128, 96));
        assert_eq!(got.material_id, want.material_id);
        assert_eq!(got.palette, None, "a generated level has no custom palette (level.cpp:102-103)");
        assert_eq!(got.display, None);
        assert_eq!(ra.last(), rb.last());
    }

    #[test]
    fn file_path_keeps_level_and_palette_draws_nothing_and_still_shadows() {
        let (large, textures, flags) = real_tc();
        let assets = LevelGenAssets { large_sprites: &large, textures: &textures, material_flags: &flags };
        // tc.cfg: 160 = see-shadow background, 12 = dirt (DirtRock), 40 = no flags.
        let mut ids = vec![40u8; 64];
        ids[6 * 8] = 160; // (0,6)
        ids[3 + 3 * 8] = 12; // (3,3), its (x+3, y-3) neighbour
        let pal = assets::palette::Palette { entries: [assets::palette::Color { r: 4, g: 8, b: 12 }; 256] };
        let file = LevelData { width: 8, height: 8, material_id: ids, palette: Some(pal.clone()), display: None };
        let params = LevelGenParams { random_level: false, random_map_width: 504, random_map_height: 350, shadow: true };
        let mut rand = seeded(5);
        let got = generate_from_settings(&assets, &params, Some(file), &mut rand);
        assert_eq!(rand.draws(), 0, "a loaded level consumes no RNG");
        assert_eq!((got.width, got.height), (8, 8));
        assert_eq!(got.material_id[6 * 8], 164, "MakeShadow applies to loaded levels (level.cpp:426-428)");
        assert_eq!(got.palette, Some(pal), "the file's palette is kept");
    }

    #[test]
    fn missing_file_falls_back_to_random_at_the_params_size() {
        let (large, textures, flags) = real_tc();
        let assets = LevelGenAssets { large_sprites: &large, textures: &textures, material_flags: &flags };
        let params = LevelGenParams { random_level: false, random_map_width: 64, random_map_height: 64, shadow: false };
        let mut ra = seeded(3);
        let got = generate_from_settings(&assets, &params, None, &mut ra);
        let mut rb = seeded(3);
        let want = generate_random(&assets, 64, 64, &mut rb);
        assert_eq!(got.material_id, want.material_id, "level.cpp:416-418 fallback, no shadow");
        assert_eq!(ra.last(), rb.last());
        assert_eq!(got.palette, None);
    }
```

Add to the top-of-file imports of `rust/sim/src/levelgen.rs`:

```rust
use assets::level::LevelData;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: compile errors `cannot find function `level_file_name``, `generate_from_settings`, `cannot find struct `LevelGenParams``.

- [ ] **Step 3: Implement**

Insert after `LevelGenAssets` in `rust/sim/src/levelgen.rs`:

```rust
/// The C++ `Settings` fields that drive `Level::GenerateFromSettings`, same names
/// (`settings.hpp:74`, `:80`, `:89-90`). 4½a maps its `MatchConfig` onto this; the level
/// FILE is resolved ([`level_file_name`]) and loaded by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelGenParams {
    pub random_level: bool,
    pub random_map_width: i32,
    pub random_map_height: i32,
    pub shadow: bool,
}

impl Default for LevelGenParams {
    /// The C++ `Settings` defaults: random level, 504x350, shadows on.
    fn default() -> Self {
        LevelGenParams { random_level: true, random_map_width: 504, random_map_height: 350, shadow: true }
    }
}
```

Insert after `generate_random`:

```rust
/// `GenerateFromSettings`' file-name rule (`level.cpp:401-404`): append `.LEV` when the
/// name contains no `.` anywhere (the whole string, directories included).
pub fn level_file_name(level_file: &str) -> String {
    if level_file.contains('.') {
        level_file.to_string()
    } else {
        format!("{level_file}.LEV")
    }
}

/// Port of `Level::GenerateFromSettings` (`level.cpp:397-429`) with the I/O lifted out.
///
/// * `params.random_level` ⇒ [`generate_random`] at `random_map_width x random_map_height`
///   (`file` is ignored, as C++ never opens it).
/// * otherwise `file` is the caller's attempt to read + parse [`level_file_name`]`(level_file)`:
///   `Some(level)` is used as loaded — palette/display kept, **no RNG drawn**; `None` (read
///   or parse failure) falls back to [`generate_random`], the `try/catch` + `!loaded` of
///   `level.cpp:405-418`.
/// * then `params.shadow` ⇒ [`make_shadow`] on either result (`level.cpp:426-428`) — so a
///   loaded level is shadowed too.
///
/// A generated level has `palette: None` (= the TC exe palette, `level.cpp:102-103`) and
/// `display: None`. Not ported: the `old_*` provenance fields (`:421-424`), which only feed
/// the NEW-GAME reuse rule (4½d keeps the params beside the level), and C++ `SetPixel`'s
/// `display_valid` clear on MODERNLV levels (render-only; Rust renders no display layer).
pub fn generate_from_settings(
    assets: &LevelGenAssets,
    params: &LevelGenParams,
    file: Option<LevelData>,
    rand: &mut Rand,
) -> LevelData {
    let (mut level, palette, display) = match file {
        Some(data) if !params.random_level => (
            LevelSim {
                width: data.width,
                height: data.height,
                material_id: data.material_id,
                material_flags: *assets.material_flags,
            },
            data.palette,
            data.display,
        ),
        _ => (
            generate_random(assets, params.random_map_width, params.random_map_height, rand),
            None,
            None,
        ),
    };
    if params.shadow {
        make_shadow(&mut level);
    }
    LevelData { width: level.width, height: level.height, material_id: level.material_id, palette, display }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim levelgen::`
Expected: `test result: ok. 29 passed; 0 failed`.

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p sim`
Expected: every `sim` test passes (the pre-existing ones are untouched).

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/levelgen.rs`
Expected: no output (else run without `--check`).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/sim/src/levelgen.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "sim(4½b): generate_from_settings (random / file / fallback, MakeShadow after either)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

### Task 7 [Opus]: MILESTONE — `levelgen_golden.rs`, the full matrix bit-exact

**Files:**
- Create: `rust/oracle-tests/tests/levelgen_golden.rs`

**Interfaces:**
- Consumes: T0's `rust/oracle-tests/golden/levelgen.txt` (cherry-picked, T0 Step 9); T1–T4 `sim::levelgen::{new_level, generate_dirt_field, splat_large_sprites, scatter_stones, dig_tunnels, place_rock_formations, place_rocks, generate_random, LevelGenAssets, RockStats}`; T5 `sim::levelgen::make_shadow`; T6 `{generate_from_settings, level_file_name, LevelGenParams}`; `sim::blit::draw_dirt_effect`; `assets::level::load(&[u8]) -> Result<LevelData, LevelError>`.
- Produces: the slice gate (design done-when 1–3, 5, 6; the `shadow=0` half of 4); helpers `load_tc() -> Tc`, `seeded`, `stage`, `fnv1a`, `dig_stamps(level: &mut LevelSim, tc: &Tc, rand: &mut Rand, after_stamp: impl FnMut(&mut LevelSim, i32, i32))` reused by T9.

This test is written after the code it checks: its "red" phase is the divergence-diagnosis loop in Step 4, keyed by the stage the failure message names.

- [ ] **Step 1: Confirm the oracle is on the branch**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 log --oneline -1 -- rust/oracle-tests/golden/levelgen.txt`
Expected: the `oracle(4½b): oracle_dump_levelgen + gen script + levelgen golden` commit. If empty, stop: the controller runs T0 Step 9 first.

- [ ] **Step 2: Write the test**

Create `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/levelgen_golden.rs`:

```rust
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

use std::collections::BTreeSet;

use assets::sprite::{SpriteSet, Tga};
use assets::tc::{TcConfig, Texture};
use sim::blit::draw_dirt_effect;
use sim::levelgen::{self, make_shadow, LevelGenAssets, LevelGenParams, RockStats};
use sim::state::LevelSim;
use sim_core::rng::Rand;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

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
    Tc { large, textures: tc.textures.clone(), flags: tc.materials }
}

/// Mirror of the dumper's `Dig`: 12 dig-texture stamps at `((i*41+5) % w - 7,
/// (i*29+7) % h - 7)`, each followed by `after_stamp(level, x, y)` — nothing for
/// `shadow=0`, CorrectShadow over `Rect(x-3, y-3, x+18, y+18)` for `shadow=1` (T9).
fn dig_stamps(level: &mut LevelSim, tc: &Tc, rand: &mut Rand, mut after_stamp: impl FnMut(&mut LevelSim, i32, i32)) {
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
    let assets = LevelGenAssets { large_sprites: &tc.large, textures: &tc.textures, material_flags: &tc.flags };

    // The six stages, each checked the moment it finishes.
    let mut rand = seeded(seed);
    let mut level = levelgen::new_level(w, h, &tc.flags);
    levelgen::generate_dirt_field(&mut level, &mut rand);
    assert_eq!(stage(&level.material_id, &rand), t[5], "{ctx}: stage `field` (level.cpp:12-26)");
    levelgen::splat_large_sprites(&mut level, &tc.large, &mut rand);
    assert_eq!(stage(&level.material_id, &rand), t[6], "{ctx}: stage `splats` (level.cpp:30-71)");
    levelgen::scatter_stones(&mut level, &tc.large, &mut rand);
    assert_eq!(stage(&level.material_id, &rand), t[7], "{ctx}: stage `stones` (level.cpp:73-82)");
    levelgen::dig_tunnels(&mut level, &tc.large, &tc.textures, &mut rand);
    assert_eq!(stage(&level.material_id, &rand), t[8], "{ctx}: stage `tunnels` (level.cpp:108-135)");
    let form = levelgen::place_rock_formations(&mut level, &tc.large, &mut rand);
    assert_eq!(form, rock_stats(&t[13..16]), "{ctx}: formation count/placed/tries");
    assert_eq!(stage(&level.material_id, &rand), t[9], "{ctx}: stage `formations` (level.cpp:137-170)");
    let rocks = levelgen::place_rocks(&mut level, &tc.large, &mut rand);
    assert_eq!(rocks, rock_stats(&t[16..19]), "{ctx}: rock count/placed/tries");
    assert_eq!(stage(&level.material_id, &rand), t[10], "{ctx}: stage `rocks` (level.cpp:172-192)");

    // The composed entry point equals the staged run.
    let mut r2 = seeded(seed);
    let composed = levelgen::generate_random(&assets, w, h, &mut r2);
    assert_eq!(stage(&composed.material_id, &r2), t[10], "{ctx}: generate_random != staged run");

    // MakeShadow (no RNG).
    if shadow {
        make_shadow(&mut level);
        let shadowed = format!("{:016x}", fnv1a(&level.material_id));
        assert_eq!(shadowed, t[11], "{ctx}: MakeShadow (level.cpp:195-216)");
        assert_ne!(t[11], &t[10][..16], "{ctx}: MakeShadow changed nothing (vacuous)");
    } else {
        assert_eq!(t[11], "-", "{ctx}: shadow=0 carries no shadowed hash");
    }

    // The shell's entry point: GenerateFromSettings(random) == generation (+ MakeShadow).
    let params = LevelGenParams { random_level: true, random_map_width: w, random_map_height: h, shadow };
    let mut r3 = seeded(seed);
    let data = levelgen::generate_from_settings(&assets, &params, None, &mut r3);
    assert_eq!(data.material_id, level.material_id, "{ctx}: generate_from_settings != staged (+ shadow)");
    assert_eq!(r3.last(), rand.last(), "{ctx}: generate_from_settings RNG position");
    assert!(data.palette.is_none() && data.display.is_none(), "{ctx}: generated level carries no palette/display");

    // Dig stage, continuing the generation RNG. shadow=0: DrawDirtEffect 7 only. shadow=1 adds
    // CorrectShadow (4½a's port) and is checked by `levelgen_dig_stage_with_correct_shadow` (T9).
    if !shadow {
        dig_stamps(&mut level, tc, &mut rand, |_, _, _| {});
        assert_eq!(stage(&level.material_id, &rand), t[12], "{ctx}: dig stage (blit.cpp:534-622)");
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
    let shadow = t[3] == "1";
    let w: i32 = t[4].parse().expect("w");
    let h: i32 = t[5].parse().expect("h");
    let ctx = format!("line {n} (file {level_file} seed {seed} shadow {})", t[3]);
    let assets = LevelGenAssets { large_sprites: &tc.large, textures: &tc.textures, material_flags: &tc.flags };

    // The caller-side I/O of GenerateFromSettings: name rule, read, parse; any failure => None.
    let name = levelgen::level_file_name(level_file);
    let file = std::fs::read(format!("{ROOT}/{name}")).ok().and_then(|b| assets::level::load(&b).ok());
    let loaded = file.is_some();
    let params = LevelGenParams { random_level: false, shadow, ..LevelGenParams::default() };
    let mut rand = seeded(seed);
    let data = levelgen::generate_from_settings(&assets, &params, file, &mut rand);
    assert_eq!((data.width, data.height), (w, h), "{ctx}: size");
    assert_eq!(stage(&data.material_id, &rand), t[6], "{ctx}: GenerateFromSettings (level.cpp:397-429)");
    if loaded {
        assert_eq!(rand.last(), 0, "{ctx}: the file branch draws no RNG");
        cov.file_loaded += 1;
    } else {
        cov.file_fallback += 1;
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
    let sizes = [(504, 350), (600, 350), (64, 64), (128, 96), (101, 77), (2000, 72), (72, 1000)];
    for (w, h) in sizes {
        for shadow in [false, true] {
            assert!(cov.combos.contains(&(w, h, shadow)), "golden lacks {w}x{h} shadow {shadow}");
        }
    }
    assert!(cov.gen_lines >= 42, "expected >= 42 gen lines, got {}", cov.gen_lines);
    // amended 2026-09-10 (controller ruling, T0 review): 5 -> 7 file lines — added the
    // levelgen_shadow_fixture.lev MakeShadow pair (shipped Levels/*.lev are MakeShadow no-ops).
    assert_eq!(cov.file_lines, 7, "file lines");
    assert!(cov.capped > 0, "no case hit the kMaxTries cap (level.cpp:137-158)");
    assert!(cov.retried > 0, "no uncapped case rejected-then-placed a rock");
    assert!(cov.file_loaded >= 3, "file branch (no RNG) not exercised");
    assert!(cov.file_fallback >= 2, "missing-file fallback not exercised");
}
```

- [ ] **Step 3: Run the gate**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test levelgen_golden`
Expected: `test levelgen_matches_cpp_oracle ... ok` and `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 4: If it fails — diagnose by the named stage (never edit the golden to match)**

The message names the line and the first diverging stage. Compare the Rust fn against the C++ **replica** function of the same name in `src/tools/oracle_dump/levelgen_dump.cpp` (itself proven equal to `level.cpp` by the dumper's self-checks):

| Failing stage | Hash wrong, `rand.last` right | `rand.last` wrong |
|---|---|---|
| `field` | arithmetic: `>> 1` vs `/ 2` are equal here; check `(left + up + rand(8) + 12) / 3` operand set | loop order: column x=0 BEFORE row y=0 (`level.cpp:14-20`) |
| `splats` | blend range `> 176 && < 180`; `(src + dest) / 2` in `u32`; zero texel skipped | per-splat draw order `x, y, frame`; `count = rand(100)` |
| `stones` | `blit_stone` clip height is `h` not `h - 1`; `mem` offset for negative origins | frame `rand(4) + 56`; draw order `x, y, frame` |
| `tunnels` | texture index `1`; top-left is `(cx, cy)` (no `-7`) | a stamp that skips `rand(r_frame)` when clipped; jitter drawn AFTER the backtrack |
| `formations` / `rocks` (stats differ) | `is_no_rock` window `size + 1`; clip | `tries` increments only on rejection; on the cap no `rand(3)` / `rand(6)` |
| `<shadowed>` | x-outer/y-inner; rule 2 re-reads the rule-1 result; bottom row covers all x | (no RNG) |
| `dig` (shadow=0) | stamp positions `(i*41+5) % w - 7`, `(i*29+7) % h - 7`; texture 7 (carving) | stamp count 12 |
| `file` | `MakeShadow` after a load; `.LEV` rule | fallback size = params (504×350) |

Fix the Rust code (T1–T6 files), rerun Step 3, and add a unit test in the owning module that pins the found cause (the Step-2 practice: every real find gets a named regression test).

- [ ] **Step 5: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/levelgen_golden.rs`
(also `add` any T1–T6 file fixed in Step 4)
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "test(4½b): MILESTONE levelgen golden bit-exact — every stage, MakeShadow, dig stage, file paths" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed` (more if Step 4 fixed code).

---

### Task 8 [Sonnet]: eyeball example, README, PROGRESS, full re-diff

**Files:**
- Create: `rust/oracle-tests/examples/levelgen_snapshot.rs`
- Modify: `rust/README.md:32` (insert a section after the "Level golden" section)
- Modify: `docs/superpowers/liero-rs-PROGRESS.md:11` (new "Last updated" paragraph) and `:713-714` (the 4½b line)

**Interfaces:**
- Consumes: T6 `sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams}`; `assets::sprite::Tga { palette: Palette, .. }`; T7's passing gate.
- Produces: `cargo run -p oracle-tests --example levelgen_snapshot -- <seed> [<w> <h>] [noshadow]` → `rust/target/snapshots/levelgen_<seed>_<w>x<h>[_noshadow].bmp`.

- [ ] **Step 1: Write the example**

Create `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/examples/levelgen_snapshot.rs`:

```rust
//! Eyeball a generated level: run `sim::levelgen::generate_from_settings` for a seed and
//! size and write the material map 1:1 as a BMP through `small.tga`'s embedded palette
//! (= C++ `common.exepal`). Dev tool only — `tests/levelgen_golden.rs` is the correctness
//! gate; this only answers "does it look like a Liero level?".
//!
//! Usage: cargo run -p oracle-tests --example levelgen_snapshot -- <seed> [<w> <h>] [noshadow]
//!   e.g. cargo run -p oracle-tests --example levelgen_snapshot -- 42 600 350
//! Output: rust/target/snapshots/levelgen_<seed>_<w>x<h>[_noshadow].bmp

use std::io::Write as _;

use assets::palette::Palette;
use assets::sprite::{SpriteSet, Tga};
use assets::tc::TcConfig;
use sim::levelgen::{generate_from_settings, LevelGenAssets, LevelGenParams};
use sim_core::rng::Rand;

/// 24-bit bottom-up BMP (same dependency-free writer as `render_snapshot.rs`, 1:1 scale).
fn write_bmp(path: &str, w: usize, h: usize, ids: &[u8], pal: &Palette) {
    let row_bytes = w * 3;
    let pad = (4 - row_bytes % 4) % 4;
    let data_size = (row_bytes + pad) * h;
    let file_size = 54 + data_size;
    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = pal.entries[ids[y * w + x] as usize];
            out.extend_from_slice(&[c.b, c.g, c.r]);
        }
        out.extend_from_slice(&vec![0u8; pad]);
    }
    let mut f = std::fs::File::create(path).expect("create bmp");
    f.write_all(&out).expect("write bmp");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed: u32 = args.first().map(|s| s.parse().expect("seed is a u32")).unwrap_or(42);
    let (w, h): (i32, i32) = if args.len() >= 3 && args[1] != "noshadow" {
        (args[1].parse().expect("width"), args[2].parse().expect("height"))
    } else {
        (504, 350)
    };
    let shadow = !args.iter().any(|a| a == "noshadow");

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let tc_root = format!("{root}/data/TC/openliero");
    let tc_bytes = std::fs::read(format!("{tc_root}/tc.cfg")).expect("read tc.cfg");
    let tc = TcConfig::load(&tc_bytes).expect("tc.cfg parses");
    let large_bytes = std::fs::read(format!("{tc_root}/sprites/large.tga")).expect("read large.tga");
    let large_tga = Tga::load(&large_bytes).expect("large.tga parses");
    let large = SpriteSet::from_tga(&large_tga, 16, 16, 110).expect("large sprite bank");
    let small_bytes = std::fs::read(format!("{tc_root}/sprites/small.tga")).expect("read small.tga");
    let small_tga = Tga::load(&small_bytes).expect("small.tga parses");

    let assets = LevelGenAssets { large_sprites: &large, textures: &tc.textures, material_flags: &tc.materials };
    let params = LevelGenParams { random_level: true, random_map_width: w, random_map_height: h, shadow };
    let mut rand = Rand::new();
    rand.seed(seed);
    let level = generate_from_settings(&assets, &params, None, &mut rand);

    let out_dir = format!("{root}/rust/target/snapshots");
    std::fs::create_dir_all(&out_dir).expect("mkdir snapshots");
    let suffix = if shadow { "" } else { "_noshadow" };
    let path = format!("{out_dir}/levelgen_{seed}_{w}x{h}{suffix}.bmp");
    write_bmp(&path, w as usize, h as usize, &level.material_id, &small_tga.palette);
    println!("{path}");
}
```

- [ ] **Step 2: Run it**

Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example levelgen_snapshot -- 42`
Expected: prints `…/rust/target/snapshots/levelgen_42_504x350.bmp`.
Run: `cargo run --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --example levelgen_snapshot -- 1 128 96 noshadow`
Expected: prints `…/levelgen_1_128x96_noshadow.bmp`.
Report both paths to the controller for John to eyeball (dirt field with carved tunnels, rock clusters, darker shadows under rock on the shadow variant). Do not treat the BMP as a gate.

Run: `rustfmt --edition 2021 --check /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/examples/levelgen_snapshot.rs /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/levelgen_golden.rs`
Expected: no output (else run without `--check` — both files are new).

- [ ] **Step 3: README section**

In `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/README.md`, replace

```markdown
The lightweight `rust.yml` CI does not regenerate it; it runs `cargo test` against
the committed `golden/level.txt`.
```

with

````markdown
The lightweight `rust.yml` CI does not regenerate it; it runs `cargo test` against
the committed `golden/level.txt`.

### Levelgen golden

`golden/levelgen.txt` pins random level generation (`sim::levelgen`) against the real
C++ `Level::GenerateRandom` / `MakeShadow` / `GenerateFromSettings`: per case, the level
hash and `rand.last` after every stage, the rock-loop statistics, and the file/fallback
paths; its dig stage is also a function-level oracle for `CorrectShadow`
(`sim::shadow`). It also needs the full C++ build:

```bash
bash rust/oracle-tests/gen_levelgen_golden.sh   # PRESET=linux-x64 on Linux
```

Eyeball a generated level (writes `rust/target/snapshots/levelgen_*.bmp`):

```bash
cargo run -p oracle-tests --example levelgen_snapshot -- <seed> [<w> <h>] [noshadow]
```
````

- [ ] **Step 4: PROGRESS**

In `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/docs/superpowers/liero-rs-PROGRESS.md`, replace

```
├─ ⬜ 4½b  random level generation + MakeShadow + SelectSpawn, dedicated Rand seeded from the
│          match seed. Gate: new oracle_dump_levelgen, bit-exact   (parallel with 4½a)   not started
```

with

```
├─ ✅ 4½b  random level generation — sim::levelgen (GenerateRandom stage by stage + MakeShadow
│          + GenerateFromSettings random/file/fallback; takes &mut Rand, the shell seeds it
│          from the match seed). Gate: oracle_dump_levelgen, 42 gen + 7 file lines BIT-EXACT
│          (amended 2026-09-10, controller ruling, T0 review: +levelgen_shadow_fixture.lev)
│          (per-stage hash + rand.last, kMaxTries cap + retry paths); its dig stage checks
│          4½a's CorrectShadow once that lands (T9). SelectSpawn deferred with Holdazone. done
```

(If T0 extended the seed list, write its gen-line count instead of 42 — T0's commit message records it.)

Then insert, directly above the line starting `> **Last updated:** 2026-09-10 · **📐 STEP 4½ (game shell) PLANNED`, a new paragraph dated with the **actual date of this commit** (check `date +%F`), and change that old line's `**Last updated:** 2026-09-10 ·` to `Prior (2026-09-10):`:

```
> **Last updated:** <today's date> · **🗺️ 4½b (random level generation) DONE — bit-exact.** The
> Rust generator (`sim::levelgen`) reproduces C++ `GenerateRandom` stage by stage (noise field,
> splats, stones, worm tunnels, rock formations, rocks — level hash AND `rand.last` after each)
> over 3 seeds × 7 sizes (incl. maps small enough to hit the `kMaxTries` cap) × shadow on/off,
> plus `MakeShadow` and `GenerateFromSettings`' file and missing-file paths. The golden's dig
> stage is a function-level oracle for 4½a's `CorrectShadow` port (checked by 4½b T9 once it
> lands). SelectSpawn deferred with Holdazone. Spec
> `specs/2026-09-10-liero-rs-step4.5-slice4.5b-level-generation-design.md`.
```

(If 4½a has already rewritten this header, keep its paragraph and put this one above it; merge conflicts here are prose-only — keep both.)

- [ ] **Step 5: Full re-diff (the standing gates)**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 diff --stat 9e41a22 -- rust/oracle-tests/golden/`
Expected: exactly one line, `rust/oracle-tests/golden/levelgen.txt | … +` (a pure addition; 9e41a22 is the Step-4½ planning commit this branch started from). If 4½a has landed on the branch meanwhile, its own new goldens may also appear — but no pre-existing golden may show a change.

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml --workspace --exclude game`
Expected: all test binaries `ok`, including `levelgen_golden` and every prior `*_golden`.

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game`
Expected: `ok` (the `game` crate is untouched; this proves the new `sim` modules did not break it).

Run: `cargo build --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p game --target wasm32-unknown-unknown`
Expected: `Finished` (the new `sim` modules do no I/O and compile for wasm).

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 log --oneline 9e41a22.. -- rust/sim/src/state.rs rust/sim/src/blit.rs rust/sim/src/shadow.rs rust/sim-core rust/scenario src/tools/oracle_dump/sim_physics_dump.cpp`
Expected: no commit whose subject contains `4½b` (file-disjointness with 4½a; commits from 4½a itself may be listed if it has landed).

- [ ] **Step 6: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/examples/levelgen_snapshot.rs rust/README.md docs/superpowers/liero-rs-PROGRESS.md`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "docs(4½b): levelgen snapshot example, README golden section, PROGRESS — 4½b done" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `3 files changed`.

---

### Task 9 [Opus]: `CorrectShadow` function-level check (gated on 4½a's port)

Run only after 4½a's `sim::shadow::correct_shadow` is on `liero-rs-step-4-5` (4½a plan T3). 4½b is complete without this task; it adds an isolated, bit-exact check of the `CorrectShadow` pixel rule (`blit.cpp:624-639`) on generated terrain, complementing 4½a's in-match shadow-on sim goldens.

**Files:**
- Modify: `rust/oracle-tests/tests/levelgen_golden.rs` (one import; one new `#[test]` after `levelgen_matches_cpp_oracle`)

**Interfaces:**
- Consumes: 4½a's `sim::shadow::correct_shadow(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32)` (4½a design §5.1; its return value is discarded, so `()` or a count both work); T7's `load_tc`, `seeded`, `stage`, `dig_stamps`, `Tc`; T4 `generate_random`; T5 `make_shadow`.
- Produces: the `shadow=1` half of design done-when 4.

- [ ] **Step 1: Confirm 4½a's port is on the branch**

Run: `grep -n "pub fn correct_shadow" /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/sim/src/shadow.rs`
Expected: one line declaring `correct_shadow(level: &mut LevelSim, x1: i32, y1: i32, x2: i32, y2: i32…`. If the file is missing, stop (4½a T3 has not landed). If the parameter list differs, adapt the single call in Step 2 to it and note the difference in the commit message.

- [ ] **Step 2: Write the test**

In `/Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/oracle-tests/tests/levelgen_golden.rs`, add below `use sim::levelgen::{…};`:

```rust
use sim::shadow::correct_shadow;
```

and append at the end of the file:

```rust
/// 4½b T9 — the `shadow=1` dig tokens through 4½a's `CorrectShadow` port: after generation
/// + MakeShadow, 12 dig stamps each followed by CorrectShadow over `Rect(x-3, y-3, x+18,
/// y+18)` (worm.cpp:931-934), continuing the generation RNG. Non-vacuity: on at least one
/// line the same stamps without CorrectShadow give a different map.
#[test]
fn levelgen_dig_stage_with_correct_shadow_matches_cpp() {
    let golden = std::fs::read_to_string(format!("{ROOT}/rust/oracle-tests/golden/levelgen.txt"))
        .expect("read golden/levelgen.txt");
    let tc = load_tc();
    let assets = LevelGenAssets { large_sprites: &tc.large, textures: &tc.textures, material_flags: &tc.flags };
    let mut checked = 0usize;
    let mut changed = 0usize;
    for (i, line) in golden.lines().enumerate() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first().copied() != Some("gen") || t[4] != "1" {
            continue;
        }
        let seed: u32 = t[1].parse().expect("seed");
        let w: i32 = t[2].parse().expect("w");
        let h: i32 = t[3].parse().expect("h");
        let ctx = format!("line {} (gen seed {seed} {w}x{h} shadow 1)", i + 1);
        let prepare = |rand: &mut Rand| {
            let mut l = levelgen::generate_random(&assets, w, h, rand);
            make_shadow(&mut l);
            l
        };

        let mut rand = seeded(seed);
        let mut level = prepare(&mut rand);
        dig_stamps(&mut level, &tc, &mut rand, |l, x, y| {
            correct_shadow(l, x - 3, y - 3, x + 18, y + 18);
        });
        assert_eq!(stage(&level.material_id, &rand), t[12], "{ctx}: dig stage with CorrectShadow (blit.cpp:624-639)");

        let mut r2 = seeded(seed);
        let mut plain = prepare(&mut r2);
        dig_stamps(&mut plain, &tc, &mut r2, |_, _, _| {});
        if plain.material_id != level.material_id {
            changed += 1;
        }
        checked += 1;
    }
    assert!(checked >= 21, "expected >= 21 shadow=1 gen lines, got {checked}");
    assert!(changed > 0, "CorrectShadow never changed a dig result (vacuous)");
}
```

- [ ] **Step 3: Run it**

Run: `cargo test --manifest-path /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5/rust/Cargo.toml -p oracle-tests --test levelgen_golden`
Expected: `test result: ok. 2 passed; 0 failed`.

If `levelgen_dig_stage_with_correct_shadow_matches_cpp` fails while `levelgen_matches_cpp_oracle` passes, the divergence is in 4½a's `correct_shadow` (generation, MakeShadow and the stamps are already proven): check the intersect with `Rect(0, 3, w - 3, h)`, the x-outer/y-inner order, the `(x+3, y-3)` neighbour, `+4` wrapping, and that the un-shadow `164..=167 && !DirtRock` is an `else if` (design §5 table). Report it to the 4½a owner with the failing line; do not change the golden.

- [ ] **Step 4: Commit**

Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 add rust/oracle-tests/tests/levelgen_golden.rs`
Run: `git -C /Users/john/code/openliero/.claude/worktrees/liero-rs-step-4-5 commit -m "test(4½b): CorrectShadow bit-exact on the levelgen dig stage (via 4½a's port)" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`
Expected: `1 file changed`.

---

## Self-review (done at plan time)

- **Spec coverage:** design done-when 1 → T1–T4 + T7; 2 → T0 matrix/guards + T7 guards; 3 → T5 + T7 `<shadowed>`; 4 → T0 dig stage + T7 (`shadow=0`) + T9 (`shadow=1`, after 4½a); 5 → T6 + T0/T7 `file` lines; 6 → T0 + T7; 7 → T8 Step 5 (+ Global Constraints); 8 → T8 Steps 1–2. Deferrals (SelectSpawn, CorrectShadow port/wiring = 4½a, reuse rule, I/O, seeding, preview) have no 4½b task by design (design §Deferrals).
- **Type consistency:** `new_level(i32, i32, &[u8;256]) -> LevelSim`; stage fns `(&mut LevelSim, [&SpriteSet], [&[Texture]], &mut Rand)`; `RockStats { count: i32, placed: i32, tries: u64 }` (C++ `int`/`int`/`uint64_t`, golden `%d %d %llu`); `LevelGenAssets { large_sprites, textures, material_flags }`; `LevelGenParams { random_level, random_map_width, random_map_height, shadow }`; `make_shadow(&mut LevelSim)`; `generate_random -> LevelSim`, `generate_from_settings -> LevelData`; 4½a's `correct_shadow(&mut LevelSim, i32, i32, i32, i32)` — used identically in T4–T9.
- **Test counts:** `levelgen::` 3 (T1) → 10 (T2) → 13 (T3) → 19 (T4) → 24 (T5) → 29 (T6); `levelgen_golden` 1 (T7) → 2 (T9).
- **Cross-slice files:** 4½b never creates or edits `rust/sim/src/shadow.rs` (4½a's); the only shared file is `rust/sim/src/lib.rs`, where 4½b adds `pub mod levelgen;` (after `hash`) and 4½a adds `pub mod shadow;` (after `pool`) — non-adjacent lines, a clean merge. PROGRESS.md edits are prose; on conflict keep both paragraphs.

