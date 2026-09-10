// Generates the golden record for the Rust random-level-generation differential test
// (Step 4½, slice 4½b; rust/oracle-tests/tests/levelgen_golden.rs). Runs the REAL C++
// generator — Level::GenerateRandom (level.cpp:101-193), Level::MakeShadow (:195-216),
// Level::GenerateFromSettings (:397-429) — plus a stage-by-stage REPLICA of
// GenerateRandom whose only purpose is to emit intermediate hashes for diagnosis. The
// replica calls the real primitives (Level::SetPixel/Pixel/Mat, DrawDirtEffect, BlitStone,
// stone_tab) plus a copy of the file-static IsNoRock (verbatim except that it drops the
// unused Common& parameter), and is SELF-CHECKED on every case against
// Level::GenerateDirtPattern, Level::GenerateRandom and Level::GenerateFromSettings (hash AND
// rand.last). Any mismatch exits 1 before argv[1] is opened, so a replica bug can never
// reach the golden: the replica slices the oracle's work, it is not the oracle.
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
// paths). Output is buffered and argv[1] is written only after every case passed; it
// exits 1 without writing unless some case hit the kMaxTries cap AND some uncapped case
// retried a rock placement (the matrix must exercise both loop exits). Built via
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

// Copy of the file-static IsNoRock (level.cpp:85-99): verbatim except that it drops the
// unused Common& parameter and renames the loop variables that shadowed x/y.
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

Coverage DumpGen(std::string& out, Common& common, uint32_t seed, int w, int h, bool shadow) {
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

  char line[512];
  std::snprintf(line, sizeof line,
                "gen %u %d %d %d %s %s %s %s %s %s %s %s %d %d %" PRIu64 " %d %d %" PRIu64 "\n",
                seed, w, h, shadow ? 1 : 0, kField.c_str(), kSplats.c_str(), kStones.c_str(),
                kTunnels.c_str(), kFormations.c_str(), kRocks.c_str(), shadowed.c_str(),
                kDig.c_str(), kForm.count, kForm.placed, kForm.tries, kRock.count, kRock.placed,
                kRock.tries);
  out += line;

  Coverage c;
  c.capped = kForm.placed < kForm.count || kRock.placed < kRock.count;
  c.retried = !c.capped && (kForm.tries > static_cast<uint64_t>(kForm.count) ||
                            kRock.tries > static_cast<uint64_t>(kRock.count));
  return c;
}

void DumpFile(std::string& out, Common& common, char const* level_file, uint32_t seed,
              bool shadow) {
  Settings settings;
  settings.random_level = false;
  settings.level_file = level_file;
  settings.shadow = shadow;  // random_map_width/height stay 504x350 for the fallback
  Level level(common);
  Rand rand;
  rand.Seed(seed);
  level.GenerateFromSettings(common, settings, rand);
  char line[256];
  std::snprintf(line, sizeof line, "file %s %u %d %d %d %s\n", level_file, seed, shadow ? 1 : 0,
                level.width, level.height, Stage(level, rand).c_str());
  out += line;
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

  std::string out;
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

  std::fprintf(stderr, "oracle_dump_levelgen: %d capped cases, %d retried cases\n", capped,
               retried);
  if (capped == 0 || retried == 0) {
    std::fprintf(stderr, "oracle_dump_levelgen: matrix misses the cap or the retry path\n");
    return 1;
  }

  std::FILE* file = std::fopen(argv[1], "w");
  if (!file) {
    std::fprintf(stderr, "cannot open %s\n", argv[1]);
    return 1;
  }
  bool const kWrote = std::fputs(out.c_str(), file) >= 0;
  if (std::fclose(file) != 0 || !kWrote) {
    std::fprintf(stderr, "cannot write %s\n", argv[1]);
    return 1;
  }
  return 0;
}
