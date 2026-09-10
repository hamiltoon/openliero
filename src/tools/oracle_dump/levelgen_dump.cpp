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
// positions drawn by the rock loops (loop iterations), summed. <level_file> is the
// repo-root-relative string passed as settings.level_file.
//
// No shipped level has a pixel MakeShadow changes, so the dumper also WRITES a fixture,
// rust/oracle-tests/golden/levelgen_shadow_fixture.lev: GenerateRandom's pre-shadow map for
// seed 42 at 128x96 in the OLLEVEL2 format (level.cpp:231-251). Its two file lines (shadow 1
// and 0) make the file branch's MakeShadow wiring a check that can fail.
//
// Run from the repo root (Common::load("data/TC/openliero") and the file cases' relative
// paths). Output is buffered and argv[1] is written only after every case passed; it
// exits 1 without writing unless some case hit the kMaxTries cap AND some uncapped case
// retried a rock placement (the matrix must exercise both loop exits), and unless
// MakeShadow changes the fixture on the file branch. Built via
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
#include "io/stream.hpp"
#include "level.hpp"
#include "math/rect.hpp"
#include "rand.hpp"
#include "settings.hpp"

namespace {

constexpr char kShadowFixture[] = "rust/oracle-tests/golden/levelgen_shadow_fixture.lev";

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

// Writes size bytes to path (binary); false on any open/write/close error.
bool WriteFile(char const* path, void const* data, std::size_t size) {
  std::FILE* file = std::fopen(path, "wb");
  if (!file) {
    return false;
  }
  bool const kWrote = std::fwrite(data, 1, size, file) == size;
  return std::fclose(file) == 0 && kWrote;
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
  int const kLen = std::snprintf(
      line, sizeof line,
      "gen %u %d %d %d %s %s %s %s %s %s %s %s %d %d %" PRIu64 " %d %d %" PRIu64 "\n", seed, w, h,
      shadow ? 1 : 0, kField.c_str(), kSplats.c_str(), kStones.c_str(), kTunnels.c_str(),
      kFormations.c_str(), kRocks.c_str(), shadowed.c_str(), kDig.c_str(), kForm.count,
      kForm.placed, kForm.tries, kRock.count, kRock.placed, kRock.tries);
  if (kLen < 0 || static_cast<std::size_t>(kLen) >= sizeof line) {
    Fail("gen line truncated", seed, w, h);
  }
  out += line;

  Coverage c;
  c.capped = kForm.placed < kForm.count || kRock.placed < kRock.count;
  c.retried = !c.capped && (kForm.tries > static_cast<uint64_t>(kForm.count) ||
                            kRock.tries > static_cast<uint64_t>(kRock.count));
  return c;
}

// Appends one file line; returns the final level hash (for the fixture guard).
std::string DumpFile(std::string& out, Common& common, char const* level_file, uint32_t seed,
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
  int const kLen =
      std::snprintf(line, sizeof line, "file %s %u %d %d %d %s\n", level_file, seed, shadow ? 1 : 0,
                    level.width, level.height, Stage(level, rand).c_str());
  if (kLen < 0 || static_cast<std::size_t>(kLen) >= sizeof line) {
    Fail("file line truncated", seed, level.width, level.height);
  }
  out += line;
  return HashOnly(level);
}

// The file-branch MakeShadow fixture: GenerateRandom's pre-shadow map for the `gen 42 128 96`
// case, serialised as OLLEVEL2 ("OLLEVEL2", version 0, width and height as u16 LE, then
// width*height material ids; level.cpp:231-251) to kShadowFixture and loaded back through
// the real GenerateFromSettings file branch with shadow 1 and 0. Everything that can be
// checked in memory is checked before the fixture is written.
void DumpShadowFixture(std::string& out, Common& common) {
  constexpr uint32_t kSeed = 42U;
  constexpr int kW = 128;
  constexpr int kH = 96;
  Settings settings;
  settings.random_map_width = kW;
  settings.random_map_height = kH;
  Level level(common);
  Rand rand;
  rand.Seed(kSeed);
  level.GenerateRandom(common, settings, rand);
  std::string const kPre = HashOnly(level);

  std::vector<uint8_t> bytes = {'O', 'L', 'L', 'E', 'V', 'E', 'L', '2'};
  bytes.push_back(0);  // version
  bytes.push_back(static_cast<uint8_t>(kW & 0xff));
  bytes.push_back(static_cast<uint8_t>((kW >> 8) & 0xff));
  bytes.push_back(static_cast<uint8_t>(kH & 0xff));
  bytes.push_back(static_cast<uint8_t>((kH >> 8) & 0xff));
  bytes.insert(bytes.end(), level.material_id.begin(), level.material_id.end());

  {  // Preflight: the bytes load back to the same map, and MakeShadow changes it.
    Level loaded(common);
    io::MemReader r(bytes);
    if (!loaded.load(common, settings, r) || HashOnly(loaded) != kPre) {
      Fail("shadow fixture does not round-trip through Level::load", kSeed, kW, kH);
    }
    loaded.MakeShadow(common);
    if (HashOnly(loaded) == kPre) {
      Fail("MakeShadow does not change the shadow fixture", kSeed, kW, kH);
    }
  }
  if (!WriteFile(kShadowFixture, bytes.data(), bytes.size())) {
    Fail("cannot write rust/oracle-tests/golden/levelgen_shadow_fixture.lev", kSeed, kW, kH);
  }

  std::string const kOn = DumpFile(out, common, kShadowFixture, kSeed, /*shadow=*/true);
  std::string const kOff = DumpFile(out, common, kShadowFixture, kSeed, /*shadow=*/false);
  if (kOff != kPre) {
    Fail("shadow fixture: shadow 0 file line != generated map (load failed?)", kSeed, kW, kH);
  }
  if (kOn == kOff) {
    Fail("shadow fixture: shadow 1 file line == shadow 0 file line", kSeed, kW, kH);
  }
  if (kOn == kPre) {
    Fail("shadow fixture: shadow 1 file line == its pre-shadow hash", kSeed, kW, kH);
  }
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
  static constexpr Size kSizes[] = {
      {.w = 504, .h = 350},  // default
      {.w = 600, .h = 350},  // test_random_map_size.cpp:97
      {.w = 64, .h = 64},    // menu minimum
      {.w = 128, .h = 96},   // "overflows with rocks" (test_random_map_size.cpp:95-96)
      {.w = 101, .h = 77},   // odd
      {.w = 2000, .h = 72},  // wide
      {.w = 72, .h = 1000},  // tall
  };
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
  DumpFile(out, *common, "data/TC/openliero/Levels/see_shadow_test.lev", 1U, /*shadow=*/true);
  DumpFile(out, *common, "data/TC/openliero/Levels/see_shadow_test.lev", 1U, /*shadow=*/false);
  DumpFile(out, *common, "data/TC/openliero/Levels/render_stage.lev", 1U, /*shadow=*/true);
  DumpFile(out, *common, "data/TC/openliero/Levels/does_not_exist", 42U, /*shadow=*/true);
  DumpFile(out, *common, "data/TC/openliero/Levels/does_not_exist", 42U, /*shadow=*/false);

  std::fprintf(stderr, "oracle_dump_levelgen: %d capped cases, %d retried cases\n", capped,
               retried);
  if (capped == 0 || retried == 0) {
    std::fprintf(stderr, "oracle_dump_levelgen: matrix misses the cap or the retry path\n");
    return 1;
  }

  DumpShadowFixture(out, *common);

  if (!WriteFile(argv[1], out.data(), out.size())) {
    std::fprintf(stderr, "oracle_dump_levelgen: cannot write %s\n", argv[1]);
    return 1;
  }
  return 0;
}
