// Generates golden digests for the Rust palette differential test by running
// the REAL C++ Palette ops. Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
#include <cstdint>
#include <cstdio>
#include <vector>

#include "gfx/palette.hpp"
#include "io/stream.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// Hash a palette's r,g,b channels (256 entries * 3 bytes), matching the Rust
// Color { r, g, b } layout (C++'s 4th `unused` byte is not hashed).
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

// MUST match the Rust test's synthetic buffers exactly.
std::vector<uint8_t> Buf(int modulo) {
  std::vector<uint8_t> b(256 * 3);
  for (std::size_t i = 0; i < b.size(); ++i) {
    b[i] = static_cast<uint8_t>(i % modulo);
  }
  return b;
}

void DumpOne(std::FILE* out, std::vector<uint8_t> const& buf) {
  Palette vga;
  io::MemReader r1(buf);
  vga.Read(r1);

  Palette full;
  io::MemReader r2(buf);
  full.ReadFull(r2);

  Palette expand = vga;
  expand.ExpandToFullRange();

  std::fprintf(out, "%016llx %016llx %016llx\n",
               static_cast<unsigned long long>(HashPalette(vga)),
               static_cast<unsigned long long>(HashPalette(full)),
               static_cast<unsigned long long>(HashPalette(expand)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: oracle_dump_palette <out.txt>\n");
    return 1;
  }
  std::FILE* out = std::fopen(argv[1], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[1]);
    return 1;
  }
  DumpOne(out, Buf(64));
  DumpOne(out, Buf(256));
  std::fclose(out);
  return 0;
}
