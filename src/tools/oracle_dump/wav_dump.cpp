// Generates golden digests for the Rust WAV differential test by running the
// REAL C++ Common::load (which decodes each sounds/<name>.wav into
// original_data and calls CreateSound). Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
// Usage: oracle_dump_wav <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "mixer/mixer.hpp"

namespace {

uint64_t Fnv1a(std::vector<unsigned char> const& data) {
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char b : data) {
    h ^= b;
    h *= 0x100000001b3ULL;
  }
  return h;
}

// FNV-1a over original_data's raw bytes (a byte buffer, hashed directly).
uint64_t HashBytes(std::vector<uint8_t> const& data) {
  std::vector<unsigned char> b(data.begin(), data.end());
  return Fnv1a(b);
}

// FNV-1a over int16 samples as explicit little-endian byte pairs.
uint64_t HashSamples(std::vector<int16_t> const& s) {
  std::vector<unsigned char> b;
  b.reserve(s.size() * 2);
  for (int16_t v : s) {
    uint16_t u = static_cast<uint16_t>(v);
    b.push_back(static_cast<unsigned char>(u & 0xff));
    b.push_back(static_cast<unsigned char>((u >> 8) & 0xff));
  }
  return Fnv1a(b);
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_wav <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  for (auto const& s : common.sounds) {
    uint64_t orig_hash = HashBytes(s.original_data);
    // Silent slot (missing/invalid file): sound == nullptr, no samples. Matches
    // the Rust WavSound::default().upsampled() == empty path.
    std::size_t up_len = 0;
    uint64_t up_hash = Fnv1a({});
    if (s.sound) {
      std::vector<int16_t>& samples = SfxSoundData(s.sound);
      up_len = samples.size();
      up_hash = HashSamples(samples);
    }
    std::fprintf(out, "%s %zu %016llx %zu %016llx\n", s.name.c_str(),
                 s.original_data.size(),
                 static_cast<unsigned long long>(orig_hash), up_len,
                 static_cast<unsigned long long>(up_hash));
  }

  std::fclose(out);
  return 0;
}
