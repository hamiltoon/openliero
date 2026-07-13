// Generates a byte-faithful `.lrp` replay corpus for the Rust slice-4e `.lrp`
// reader differential test. Builds a real C++ `Game` from a scenario file using
// the SAME tick-0 setup as `sim_physics_dump.cpp` (seed RNG, load a FIXED `.lev`,
// add 2 worms, InitWeapons, ResetWorms, apply the scenario start conditions), so
// the initial `Game` this tool serialises into the `.lrp` is bit-identical to what
// Rust's `scenario::loader::load` + `SimState::new` reconstruct. It then drives the
// REAL `Game::ProcessFrame` through a `ReplayWriter`, one tick per scenario input,
// emitting the container (`LRPF` magic + version + cereal `Game`), the per-worm
// XOR-delta input stream, and the embedded `WideRollbackChecksum` word every 1050
// frames. The `.lrp` is the authoritative C++ oracle the Rust reader plays back.
//
// Why REAL `Game::ProcessFrame` (not the reduced TAIL `sim_physics_dump` uses for its
// hash goldens): the `.lrp`'s embedded `WideRollbackChecksum` words are written inside
// `ReplayWriter::RecordFrame` (replay.cpp:369-372) against the live `Game`, and the
// checksum folds the ENTIRE rollback-state inventory (RNG, cycles, every worm field,
// the four projectile/bonus pools, the whole material buffer). Only the genuine
// `ProcessFrame` — which also maintains `prev_control_states` at its tail
// (game.cpp:466-468), the XOR baseline the delta stream depends on — produces the
// authoritative trajectory. The frame loop mirrors `LocalController`: set each worm's
// `control_states`, `RecordFrame()` (writes the input delta, then the checksum word if
// `cycles % 1050 == 0`), THEN `ProcessFrame()` (input-map §1b/§4). `EndRecord` (the
// `0x83` end tag) fires in the `ReplayWriter` destructor.
//
// Headless traps (identical to `sim_physics_dump`): `NullSoundPlayer` (no audio device)
// and a BASE `StatsRecorder` via `make_shared<StatsRecorder>()` — NOT
// `NormalStatsRecorder`, whose `DamageDealt` deref crashes headless once the worm-damage
// path runs. No viewports / renderer / SDL: `WideRollbackChecksum` folds no viewport
// state and `RecordFrame` does not draw, so `ProcessViewports` over the empty
// `game.viewports` is a no-op.
//
// Scenario file (argv[1]) — the SHARED grammar `sim_physics_dump.cpp` parses (so the
// same corpus scenarios drive both sides). Only the sim-relevant directives affect the
// `.lrp`; the draw-only `render*` directives are accepted and IGNORED (this tool never
// draws):
//   seed <u32>
//   level <path relative to data/TC/openliero>
//   ticks <N>
//   max_bonuses <n>   (Settings::max_bonuses)
//   game_mode <n>     (Settings::game_mode enum; default 0 = kGmKillEmAll)
//   worm <idx> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>
//   input <tick> <worm0_7bit> <worm1_7bit>   (sparse; absent => 0)
//   weapon <slot> <name> [ammo]              (override BOTH worms' weapon slot)
//   render* ...                              (draw-only; ignored here)
//
// Usage: oracle_dump_lrp_gen <scenario.txt> <out.lrp> [seed] [hash_sidecar.txt]
// The optional 4th arg writes a per-tick HashGameState sidecar in the EXACT 11-column
// format `sim_physics_dump` emits (tick, HashGameState, then the eight component hashes),
// so the tick-0-alignment cross-check is a direct `diff` against the scenario's committed
// `_sim.txt` golden. Built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option; not part of
// the default build.
#include <array>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "common.hpp"
#include "constants.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "replay.hpp"
#include "settings.hpp"
#include "stateHash.hpp"
#include "stats_recorder.hpp"
#include "weapon.hpp"
#include "worm.hpp"

namespace {

struct WormSpec {
  int index = 0;
  int pos_x = 0;
  int pos_y = 0;
  int health = 100;
  int lives = 10;
  int stats_x = 0;
  int visible = 1;
};

struct Scenario {
  uint32_t seed = 42;
  std::string level;
  int ticks = 0;
  std::vector<WormSpec> worms;
  // tick -> packed 7-bit input per worm index.
  std::map<int, std::array<uint32_t, 2>> inputs;
  // slot -> weapon name override (applied to BOTH worms after ResetWorms).
  std::map<int, std::string> weapon_overrides;
  // slot -> opt-in ammo override (only set when the `weapon` directive has a 3rd token).
  std::map<int, int> weapon_ammo_overrides;
  // Settings::max_bonuses for the per-tick bonus-drop roll. Default 0 => the roll
  // short-circuits (no rand drawn).
  int max_bonuses = 0;
  // Settings::game_mode for the game-mode switch. Default kGmKillEmAll (0) => inert.
  int game_mode = Settings::kGmKillEmAll;
};

std::vector<uint8_t> SlurpFile(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    std::fprintf(stderr, "cannot open level %s\n", path.c_str());
    std::exit(1);
  }
  return std::vector<uint8_t>(std::istreambuf_iterator<char>(f),
                              std::istreambuf_iterator<char>());
}

Scenario ParseScenario(char const* path) {
  std::ifstream f(path);
  if (!f) {
    std::fprintf(stderr, "cannot open scenario %s\n", path);
    std::exit(1);
  }
  Scenario s;
  std::string line;
  while (std::getline(f, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key)) {
      continue;  // blank
    }
    if (key[0] == '#') {
      continue;  // comment
    }
    if (key == "seed") {
      ls >> s.seed;
    } else if (key == "level") {
      ls >> s.level;
    } else if (key == "ticks") {
      ls >> s.ticks;
    } else if (key == "max_bonuses") {
      ls >> s.max_bonuses;
    } else if (key == "game_mode") {
      ls >> s.game_mode;
    } else if (key.rfind("render", 0) == 0) {
      // Draw-only directives (render / render_shadow / render_shake / render_flash /
      // render_hud / render_live). This tool never draws — the `.lrp` folds no viewport
      // state — so they are irrelevant to the recorded trajectory and ignored whole.
    } else if (key == "worm") {
      WormSpec w;
      ls >> w.index >> w.pos_x >> w.pos_y >> w.health >> w.lives >> w.stats_x >> w.visible;
      s.worms.push_back(w);
    } else if (key == "input") {
      int tick = 0;
      std::array<uint32_t, 2> in{0, 0};
      ls >> tick >> in[0] >> in[1];
      s.inputs[tick] = in;
    } else if (key == "weapon") {
      int slot = 0;
      std::string name;
      ls >> slot >> name;
      if (slot < 0 || slot >= NUM_WEAPONS) {
        std::fprintf(stderr, "weapon slot out of range: %d\n", slot);
        std::exit(1);
      }
      s.weapon_overrides[slot] = name;
      // Optional 3rd token: opt-in low-ammo override to reach the reload branch quickly.
      int ammo = 0;
      if (ls >> ammo) {
        s.weapon_ammo_overrides[slot] = ammo;
      }
    } else {
      std::fprintf(stderr, "unknown scenario key: %s\n", key.c_str());
      std::exit(1);
    }
  }
  if (s.worms.size() != 2) {
    std::fprintf(stderr, "scenario must define exactly 2 worms (got %zu)\n", s.worms.size());
    std::exit(1);
  }
  return s;
}

// Find a weapon by name in `common->weapons`; exit(1) if unresolvable.
int ResolveWeapon(Common const& common, std::string const& name) {
  for (size_t i = 0; i < common.weapons.size(); ++i) {
    if (common.weapons[i].name == name) {
      return static_cast<int>(i);
    }
  }
  std::fprintf(stderr, "unknown weapon name: %s\n", name.c_str());
  std::exit(1);
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_lrp_gen <scenario.txt> <out.lrp> [seed] [hash.txt]\n");
    return 1;
  }
  Scenario const scn = ParseScenario(argv[1]);
  uint32_t const seed =
      (argc >= 4) ? static_cast<uint32_t>(std::strtoul(argv[3], nullptr, 10)) : scn.seed;

  PrecomputeTables();

  auto common = std::make_shared<Common>();
  FsNode const kTcRoot(FsNode("data") / "TC" / "openliero");
  common->load(kTcRoot);

  auto settings = std::make_shared<Settings>();
  settings->game_mode = scn.game_mode;
  settings->lives = scn.worms[0].lives;
  settings->loading_time = 0;
  settings->shadow = false;
  settings->max_bonuses = scn.max_bonuses;

  auto sound_player = std::make_shared<NullSoundPlayer>();
  Game game(common, settings, sound_player);
  // Install the base no-op StatsRecorder (stats_recorder.cpp:8-29): a default Game has a
  // null `stats_recorder`, and once the worm-damage path runs `DamageDealt` would deref
  // it and crash headless. The crashing subclass is `NormalStatsRecorder` — never use it.
  game.stats_recorder = std::make_shared<StatsRecorder>();
  // StartGame parity for the blood pool: the real game sizes `bobjects` from
  // `settings->blood_particle_max` in `Game::StartGame` (game.cpp:513). This tool
  // hand-rolls setup and never calls `StartGame`, so resize here to the true 700-cap pool
  // (otherwise `NewObjectReuse` collapses a blood spray to one slot). Uses no RNG.
  game.bobjects.Resize(settings->blood_particle_max);
  game.rand.Seed(seed);

  // Load a FIXED level (NOT GenerateFromSettings, which would consume RNG). The
  // scenario level path is relative to the TC root; the gen script runs from ROOT.
  {
    std::string const level_path = "data/TC/openliero/" + scn.level;
    std::vector<uint8_t> const buf = SlurpFile(level_path);
    io::MemReader r(buf);
    if (!game.level.load(*common, *settings, r)) {
      std::fprintf(stderr, "Level::load failed for %s\n", level_path.c_str());
      return 1;
    }
  }

  // Add 2 worms exactly as the determinism fixture (test_determinism.cpp), with
  // health / stats_x from the scenario.
  for (int idx = 0; idx < 2; ++idx) {
    WormSpec const& spec = scn.worms[idx];
    auto w = std::make_shared<Worm>();
    w->settings = settings->worm_settings[idx];
    w->health = spec.health;
    w->index = idx;
    w->stats_x = spec.stats_x;
    game.AddWorm(w);
  }
  for (auto const& w : game.worms) {
    w->InitWeapons(game);
  }
  game.ResetWorms();

  // Apply scenario start conditions (ResetWorms reset health/visible/lives, so set
  // them AFTER it).
  for (int idx = 0; idx < 2; ++idx) {
    WormSpec const& spec = scn.worms[idx];
    auto const& w = game.worms[idx];
    w->pos = {spec.pos_x, spec.pos_y};
    w->vel = {0, 0};
    w->health = spec.health;
    w->lives = spec.lives;
    w->visible = spec.visible != 0;

    // Apply per-slot weapon overrides (ResetWorms re-ran InitWeapons, so do this
    // after it). Set the slot ready to fire: full ammo, no delay, not loading.
    for (auto const& [slot, name] : scn.weapon_overrides) {
      WormWeapon& ww = w->weapons[slot];
      ww.type = &common->weapons[ResolveWeapon(*common, name)];
      ww.ammo = ww.type->ammo;
      auto ammo_it = scn.weapon_ammo_overrides.find(slot);
      if (ammo_it != scn.weapon_ammo_overrides.end()) {
        ww.ammo = ammo_it->second;
      }
      ww.delay_left = 0;
      ww.loading_left = 0;
    }
    w->current_weapon = 0;
  }

  // Optional HashGameState sidecar, in the EXACT 11-column `sim_physics_dump` format, for
  // the tick-0-alignment cross-check (direct `diff` against the scenario's `_sim` golden).
  std::FILE* hash_out = nullptr;
  if (argc >= 5) {
    hash_out = std::fopen(argv[4], "w");
    if (!hash_out) {
      std::fprintf(stderr, "cannot open %s\n", argv[4]);
      return 1;
    }
  }
  auto dump_hash = [&](int tick) {
    if (!hash_out) {
      return;
    }
    uint32_t const state_hash = HashGameState(game);
    ComponentHashes const c = HashGameComponents(game);
    std::fprintf(hash_out, "%d %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x\n", tick,
                 state_hash, c.rng, c.level, c.worms[0], c.worms[1], c.bobjects, c.bonuses,
                 c.sobjects, c.nobjects, c.wobjects);
  };

  // Record: `BeginRecord` writes the magic + version + cereal `Game` (the tick-0 blob).
  // Each tick mirrors `LocalController`: set control states, `RecordFrame` (input delta +
  // the checksum word when `cycles % 1050 == 0`), then the real `ProcessFrame`. Scoped so
  // the destructor writes the `0x83` end tag and flushes the deflate stream before we
  // close the sidecar / return.
  {
    ReplayWriter replay(std::make_unique<io::FileWriter>(argv[2], "wb"));
    replay.BeginRecord(game);

    dump_hash(0);
    for (int t = 0; t < scn.ticks; ++t) {
      std::array<uint32_t, 2> in{0, 0};
      auto const it = scn.inputs.find(t);
      if (it != scn.inputs.end()) {
        in = it->second;
      }
      for (int idx = 0; idx < static_cast<int>(game.worms.size()); ++idx) {
        game.worms[idx]->control_states.Unpack(idx < 2 ? in[idx] : 0);
      }
      replay.RecordFrame();
      game.ProcessFrame();
      dump_hash(t + 1);
    }
  }

  if (hash_out) {
    std::fclose(hash_out);
  }
  return 0;
}
