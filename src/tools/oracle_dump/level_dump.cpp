// Generates the golden material-map digest for the Rust level differential test.
// Runs the REAL C++ Level::load on three inputs and writes one FNV-1a hash per
// material map. Links the `game` library; built via the OPENLIERO_BUILD_ORACLE_DUMP
// CMake option (see gen_level_golden.sh). Not part of the default build.
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

void DumpOne(std::FILE* out, Common& common, Settings const& settings,
             std::vector<uint8_t> const& buf) {
  io::MemReader r(buf);
  Level level(common);
  if (!level.load(common, settings, r)) {
    std::fprintf(stderr, "Level::load failed\n");
    std::exit(1);
  }
  std::fprintf(out, "%d %d %016llx\n", level.width, level.height,
               static_cast<unsigned long long>(Fnv1a(level.material_id)));
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
  settings.load_powerlevel_palette = false;

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  DumpOne(out, common, settings, SlurpFile(argv[1]));  // real modern_test.lev
  DumpOne(out, common, settings, MakeLegacy());
  DumpOne(out, common, settings, MakeOllevel2());
  std::fclose(out);
  return 0;
}
