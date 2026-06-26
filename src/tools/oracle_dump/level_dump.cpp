// Generates the golden digests for the Rust level differential test. Runs the
// REAL C++ Level::load on seven inputs and writes one line each:
// `w h mat_hash pal_hash dd_hash dv_hash ramp_hash anim_hash` (FNV-1a hashes of
// the material map, POWERLEVEL palette, and MODERNLV display/ramp/anim fields,
// `-` for absent optional fields). Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option (see gen_level_golden.sh). Not part
// of the default build.
#include <cstdint>
#include <cstdio>
#include <fstream>
#include <iterator>
#include <vector>

#include "common.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "settings.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// Hash a palette's r,g,b channels (matches Rust Color { r, g, b }).
uint64_t HashPalette(Palette const& p) {
  std::vector<unsigned char> b;
  b.reserve(256 * 3);
  for (auto const& e : p.entries) {
    b.push_back(e.r);
    b.push_back(e.g);
    b.push_back(e.b);
  }
  return Fnv1a(b);
}

// Hash u32 values as explicit little-endian bytes (host-endian independent).
uint64_t HashU32LE(std::vector<uint32_t> const& v) {
  std::vector<unsigned char> b;
  b.reserve(v.size() * 4);
  for (uint32_t x : v) {
    b.push_back(static_cast<unsigned char>(x & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 8) & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 16) & 0xff));
    b.push_back(static_cast<unsigned char>((x >> 24) & 0xff));
  }
  return Fnv1a(b);
}

uint64_t HashBytes(std::vector<uint8_t> const& v) {
  return Fnv1a(std::vector<unsigned char>(v.begin(), v.end()));
}

// Ramp table serialized as: shift byte then colors as LE u32, per ramp.
uint64_t HashRamps(std::vector<Level::ArgbRamp> const& ramps) {
  std::vector<unsigned char> b;
  for (auto const& r : ramps) {
    b.push_back(r.shift);
    for (uint32_t c : r.colors) {
      b.push_back(static_cast<unsigned char>(c & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 8) & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 16) & 0xff));
      b.push_back(static_cast<unsigned char>((c >> 24) & 0xff));
    }
  }
  return Fnv1a(b);
}

std::vector<uint8_t> SlurpFile(char const* path) {
  std::ifstream f(path, std::ios::binary);
  return std::vector<uint8_t>(std::istreambuf_iterator<char>(f),
                              std::istreambuf_iterator<char>());
}

// MUST match the Rust test's synthetic inputs exactly.
std::vector<uint8_t> MakeLegacy() {
  std::vector<uint8_t> b(504 * 350);
  for (std::size_t i = 0; i < b.size(); ++i) {
    b[i] = static_cast<uint8_t>(i % 251);
  }
  return b;
}

std::vector<uint8_t> MakeOllevel2() {
  std::vector<uint8_t> b = {'O', 'L', 'L', 'E', 'V', 'E', 'L', '2'};
  b.push_back(0);   // version
  b.push_back(13);  // width LE
  b.push_back(0);
  b.push_back(11);  // height LE
  b.push_back(0);
  for (int i = 0; i < 13 * 11; ++i) {
    b.push_back(static_cast<uint8_t>((i * 5 + 2) % 256));
  }
  return b;
}

// A small OLLEVEL2 level (w*h cells) with the given trailing block appended.
std::vector<uint8_t> MakeSizedWith(int w, int h, std::vector<uint8_t> const& tail) {
  std::vector<uint8_t> b = {'O', 'L', 'L', 'E', 'V', 'E', 'L', '2'};
  b.push_back(0);
  b.push_back(static_cast<uint8_t>(w & 0xff));
  b.push_back(static_cast<uint8_t>((w >> 8) & 0xff));
  b.push_back(static_cast<uint8_t>(h & 0xff));
  b.push_back(static_cast<uint8_t>((h >> 8) & 0xff));
  for (int i = 0; i < w * h; ++i) {
    b.push_back(static_cast<uint8_t>((i * 5 + 2) % 256));
  }
  b.insert(b.end(), tail.begin(), tail.end());
  return b;
}

std::vector<uint8_t> Powerlevel() {
  std::vector<uint8_t> b = {'P', 'O', 'W', 'E', 'R', 'L', 'E', 'V', 'E', 'L'};
  for (int i = 0; i < 256 * 3; ++i) b.push_back(static_cast<uint8_t>(i % 64));
  return b;
}

// MODERNLV block for `cells` pixels. anim_kind: 0=none, 1=good, 2=bad index.
std::vector<uint8_t> Modernlv(int cells, int anim_kind) {
  std::vector<uint8_t> b = {'M', 'O', 'D', 'E', 'R', 'N', 'L', 'V'};
  for (int i = 0; i < cells; ++i) {
    uint32_t v = 0x11223300u + static_cast<uint32_t>(i);
    b.push_back(static_cast<uint8_t>(v & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 8) & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 16) & 0xff));
    b.push_back(static_cast<uint8_t>((v >> 24) & 0xff));
  }
  for (int i = 0; i < cells; ++i) b.push_back(static_cast<uint8_t>(i % 2));
  if (anim_kind != 0) {
    b.push_back(1);  // ramp_count
    b.push_back(3);  // shift
    b.push_back(2);  // color_count LE
    b.push_back(0);
    uint32_t cols[2] = {0xAABBCCDDu, 0x01020304u};
    for (uint32_t c : cols) {
      b.push_back(static_cast<uint8_t>(c & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 8) & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 16) & 0xff));
      b.push_back(static_cast<uint8_t>((c >> 24) & 0xff));
    }
    for (int i = 0; i < cells; ++i) {
      uint8_t idx = (anim_kind == 2 && i == 1) ? 2 : static_cast<uint8_t>(i % 2);
      b.push_back(idx);
    }
  }
  return b;
}

void DumpOne(std::FILE* out, Common& common, Settings const& settings,
             std::vector<uint8_t> const& buf) {
  io::MemReader r(buf);
  Level level(common);
  if (!level.load(common, settings, r)) {
    std::fprintf(stderr, "Level::load failed\n");
    std::exit(1);
  }
  std::fprintf(out, "%d %d %016llx", level.width, level.height,
               static_cast<unsigned long long>(Fnv1a(level.material_id)));

  if (level.has_custom_palette) {
    std::fprintf(out, " %016llx",
                 static_cast<unsigned long long>(HashPalette(level.origpal)));
  } else {
    std::fprintf(out, " -");
  }

  if (!level.display_data.empty()) {
    std::fprintf(out, " %016llx %016llx",
                 static_cast<unsigned long long>(HashU32LE(level.display_data)),
                 static_cast<unsigned long long>(HashBytes(level.display_valid)));
  } else {
    std::fprintf(out, " - -");
  }

  if (!level.argb_ramps.empty()) {
    std::fprintf(out, " %016llx %016llx",
                 static_cast<unsigned long long>(HashRamps(level.argb_ramps)),
                 static_cast<unsigned long long>(HashBytes(level.display_anim)));
  } else {
    std::fprintf(out, " - -");
  }
  std::fprintf(out, "\n");
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_level <modern.lev> <out.txt>\n");
    return 1;
  }
  Common common;
  for (auto& m : common.materials) {
    m.flags = 0;  // FillMaterials, as in test_sized_level.cpp
  }
  Settings settings;
  settings.load_powerlevel_palette = true;

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  DumpOne(out, common, settings, SlurpFile(argv[1]));  // real modern_test.lev
  DumpOne(out, common, settings, MakeLegacy());
  DumpOne(out, common, settings, MakeOllevel2());
  DumpOne(out, common, settings, MakeSizedWith(4, 4, Powerlevel()));
  DumpOne(out, common, settings, MakeSizedWith(2, 2, Modernlv(4, 0)));
  DumpOne(out, common, settings, MakeSizedWith(2, 2, Modernlv(4, 2)));
  {
    std::vector<uint8_t> tail = Powerlevel();
    auto m = Modernlv(4, 0);
    tail.insert(tail.end(), m.begin(), m.end());
    DumpOne(out, common, settings, MakeSizedWith(2, 2, tail));
  }
  std::fclose(out);
  return 0;
}
