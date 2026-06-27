// Generates golden digests for the Rust object-config differential test by
// running the REAL C++ Common::load (which reads weapons/nobjects/sobjects .cfg
// and resolves cross-references). Links the `game` library; built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option. Not part of the default build.
// Usage: oracle_dump_object <tc-dir> <out.txt>
#include <cstdint>
#include <cstdio>
#include <string>
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

void PushU32(std::vector<unsigned char>& b, uint32_t v) {
  b.push_back(static_cast<unsigned char>(v & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 8) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 16) & 0xff));
  b.push_back(static_cast<unsigned char>((v >> 24) & 0xff));
}

void PushI32(std::vector<unsigned char>& b, int32_t v) {
  PushU32(b, static_cast<uint32_t>(v));
}

void PushBool(std::vector<unsigned char>& b, bool v) {
  b.push_back(v ? 1 : 0);
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
    std::fprintf(stderr, "usage: oracle_dump_object <tc-dir> <out.txt>\n");
    return 1;
  }
  Common common;
  common.load(FsNode(argv[1]));

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  // weapons[] in LoadWeaponConfig read order (common_model.hpp:258-335),
  // prefixed per entry by id (i32) + id_str (string).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.weapons.size()));
    for (Weapon const& w : common.weapons) {
      PushI32(b, w.id);
      PushStr(b, w.id_str);
      PushStr(b, w.name);
      PushBool(b, w.affect_by_worm);
      PushBool(b, w.shadow);
      PushBool(b, w.laser_sight);
      PushBool(b, w.play_reload_sound);
      PushBool(b, w.worm_explode);
      PushBool(b, w.expl_ground);
      PushBool(b, w.worm_collide);
      PushBool(b, w.collide_with_objects);
      PushBool(b, w.affect_by_explosions);
      PushBool(b, w.loop_anim);
      PushI32(b, w.detect_distance);
      PushI32(b, w.blow_away);
      PushI32(b, w.gravity);
      PushI32(b, w.launch_sound);
      PushBool(b, w.loop_sound);  // bool (int->bool quirk)
      PushI32(b, w.explo_sound);
      PushI32(b, w.speed);
      PushI32(b, w.add_speed);
      PushI32(b, w.distribution);
      PushI32(b, w.parts);
      PushI32(b, w.recoil);
      PushI32(b, w.mult_speed);
      PushI32(b, w.delay);
      PushI32(b, w.loading_time);
      PushI32(b, w.ammo);
      PushI32(b, w.dirt_effect);
      PushI32(b, w.leave_shells);
      PushI32(b, w.leave_shell_delay);
      PushI32(b, w.fire_cone);
      PushI32(b, w.bounce);
      PushI32(b, w.time_to_explo);
      PushI32(b, w.time_to_explo_v);
      PushI32(b, w.hit_damage);
      PushI32(b, w.blood_on_hit);
      PushI32(b, w.start_frame);
      PushI32(b, w.num_frames);
      PushI32(b, w.shot_type);
      PushI32(b, w.color_bullets);
      PushI32(b, w.splinter_amount);
      PushI32(b, w.splinter_colour);
      PushI32(b, w.splinter_type);
      PushI32(b, w.splinter_scatter);
      PushI32(b, w.obj_trail_type);
      PushI32(b, w.obj_trail_delay);
      PushI32(b, w.part_trail_type);
      PushI32(b, w.part_trail_obj);
      PushI32(b, w.part_trail_delay);
      PushI32(b, w.create_on_exp);
      PushBool(b, w.chain_explosion);
    }
    Emit(out, "weapons", b);
  }

  // nobject_types[] in LoadNObjectConfig read order (common_model.hpp:98-137).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.nobject_types.size()));
    for (NObjectType const& n : common.nobject_types) {
      PushI32(b, n.id);
      PushStr(b, n.id_str);
      PushBool(b, n.worm_explode);
      PushBool(b, n.expl_ground);
      PushBool(b, n.worm_destroy);
      PushBool(b, n.draw_on_map);
      PushBool(b, n.affect_by_explosions);
      PushBool(b, n.blood_trail);
      PushI32(b, n.detect_distance);
      PushI32(b, n.gravity);
      PushI32(b, n.speed);
      PushI32(b, n.speed_v);
      PushI32(b, n.distribution);
      PushI32(b, n.blow_away);
      PushI32(b, n.bounce);
      PushI32(b, n.hit_damage);
      PushI32(b, n.blood_on_hit);
      PushI32(b, n.start_frame);
      PushI32(b, n.num_frames);
      PushI32(b, n.color_bullets);
      PushI32(b, n.create_on_exp);
      PushI32(b, n.dirt_effect);
      PushI32(b, n.splinter_amount);
      PushI32(b, n.splinter_colour);
      PushI32(b, n.splinter_type);
      PushI32(b, n.blood_trail_delay);
      PushI32(b, n.leave_obj);
      PushI32(b, n.leave_obj_delay);
      PushI32(b, n.time_to_explo);
      PushI32(b, n.time_to_explo_v);
    }
    Emit(out, "nobjects", b);
  }

  // sobject_types[] in LoadSObjectConfig read order (common_model.hpp:162-177).
  {
    std::vector<unsigned char> b;
    PushU32(b, static_cast<uint32_t>(common.sobject_types.size()));
    for (SObjectType const& s : common.sobject_types) {
      PushI32(b, s.id);
      PushStr(b, s.id_str);
      PushBool(b, s.shadow);
      PushI32(b, s.start_sound);
      PushI32(b, s.num_sounds);
      PushI32(b, s.anim_delay);
      PushI32(b, s.start_frame);
      PushI32(b, s.num_frames);
      PushI32(b, s.detect_range);
      PushI32(b, s.damage);
      PushI32(b, s.blow_away);
      PushI32(b, s.shake);
      PushI32(b, s.flash);
      PushI32(b, s.dirt_effect);
    }
    Emit(out, "sobjects", b);
  }

  std::fclose(out);
  return 0;
}
