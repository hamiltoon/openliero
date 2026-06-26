// Generates golden digests for the Rust sprite differential test by running the
// REAL C++ Common::load (which calls ReadSpriteTga). Links the `game` library;
// built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the
// default build. Usage: oracle_dump_sprite <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// Hash a palette's r,g,b channels (256 entries * 3 bytes), matching Rust
// Color { r, g, b } (C++'s 4th `unused` byte is not hashed).
uint64_t HashPalette(Palette const& p) {
  std::vector<unsigned char> bytes;
  bytes.reserve(256 * 3);
  for (auto const& e : p.entries) {
    bytes.push_back(e.r);
    bytes.push_back(e.g);
    bytes.push_back(e.b);
  }
  return Fnv1a(bytes);
}

void DumpBank(std::FILE* out, char const* label, SpriteSet const& ss) {
  std::vector<unsigned char> bytes(ss.data.begin(), ss.data.end());
  std::fprintf(out, "%s %d %d %d %016llx\n", label, ss.count, ss.width, ss.height,
               static_cast<unsigned long long>(Fnv1a(bytes)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_sprite <tc-dir> <out.txt>\n");
    return 1;
  }
  auto common = std::make_shared<Common>();
  common->load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  DumpBank(out, "small", common->small_sprites);
  DumpBank(out, "large", common->large_sprites);
  DumpBank(out, "text", common->text_sprites);
  std::fprintf(out, "exepal %016llx\n",
               static_cast<unsigned long long>(HashPalette(common->exepal)));
  std::fclose(out);
  return 0;
}
