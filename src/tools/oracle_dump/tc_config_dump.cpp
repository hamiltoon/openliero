// Generates golden digests for the Rust tc.cfg differential test by running the
// REAL C++ Common::load (which calls LoadTcConfig). Links the `game` library;
// built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the
// default build. Usage: oracle_dump_tc <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

#include "common.hpp"
#include "constants.hpp"
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

void PushU32(std::vector<unsigned char>& b, uint32_t v) {
  b.push_back(static_cast<unsigned char>(v & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 8) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 16) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 24) & 0xff));
}

void PushI32(std::vector<unsigned char>& b, int32_t v) {
  PushU32(b, static_cast<uint32_t>(v));
}

void PushStr(std::vector<unsigned char>& b, std::string const& s) {
  PushU32(b, static_cast<uint32_t>(s.size()));
  for (char c : s) {
    b.push_back(static_cast<unsigned char>(c));
  }
}

void Emit(std::FILE* out, char const* label, std::vector<unsigned char> const& b) {
  std::fprintf(out, "%s %016llx\n", label, static_cast<unsigned long long>(Fnv1a(b)));
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_tc <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sounds.size()));
    for (auto const& s : common.sounds) PushStr(b, s.name);
    Emit(out, "types_sounds", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.weapons.size()));
    for (auto const& w : common.weapons) PushStr(b, w.id_str);
    Emit(out, "types_weapons", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.nobject_types.size()));
    for (auto const& w : common.nobject_types) PushStr(b, w.id_str);
    Emit(out, "types_nobjects", b);
  }
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sobject_types.size()));
    for (auto const& w : common.sobject_types) PushStr(b, w.id_str);
    Emit(out, "types_sobjects", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_C(n) PushI32(b, common.c[C##n]);
    LIERO_CDEFS(HASH_C)
#undef HASH_C
    Emit(out, "constants", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < MAX_MATERIALS; ++i) b.push_back(common.materials[i].flags);
    Emit(out, "materials", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_TEXTURES; ++i) {
      Texture const& t = common.textures[i];
      PushI32(b, t.m_frame);
      PushI32(b, t.r_frame);
      PushI32(b, t.s_frame);
      b.push_back(t.n_draw_back ? 1 : 0);
    }
    Emit(out, "textures", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_BONUS_SOBJECTS; ++i) {
      PushI32(b, common.bonus_rand_timer[i][0]);
      PushI32(b, common.bonus_rand_timer[i][1]);
      PushI32(b, common.bonus_frames[i]);
      PushI32(b, common.bonus_s_objects[i]);
    }
    Emit(out, "bonuses", b);
  }

  {
    std::vector<unsigned char> b;
    for (int i = 0; i < NUM_COLOR_ANIM; ++i) {
      PushI32(b, common.color_anim[i].from);
      PushI32(b, common.color_anim[i].to);
    }
    Emit(out, "coloranim", b);
  }

  {
    // Engine stores k[1][idx] = on, k[0][idx] = off; idx 0..6 = up..jump.
    std::vector<unsigned char> b;
    for (int idx = 0; idx < NUM_AIPARAMS_KEYS; ++idx) {
      PushI32(b, common.ai_params.k[1][idx]);
      PushI32(b, common.ai_params.k[0][idx]);
    }
    Emit(out, "aiparams", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_S(n) PushStr(b, common.s[S##n]);
    LIERO_SDEFS(HASH_S)
#undef HASH_S
    Emit(out, "texts", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_H(n) b.push_back(common.h[H##n] ? 1 : 0);
    LIERO_HDEFS(HASH_H)
#undef HASH_H
    Emit(out, "hacks", b);
  }

  {
    std::vector<unsigned char> b;
#define HASH_SO(n) PushI32(b, common.sound_hook[Sound##n]);
    LIERO_SOUNDDEFS(HASH_SO)
#undef HASH_SO
    Emit(out, "soundhooks", b);
  }

  std::fclose(out);
  return 0;
}
