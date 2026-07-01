// Generates the golden record for the Rust sim slice-1 differential test. Builds
// a real C++ `Game` to the exact tick-0 fixture state (seed RNG, load a FIXED
// `.lev`, add 2 worms, InitWeapons, ResetWorms) and dumps both the full
// `HashGameState` and every `HashGameComponents` field BEFORE any ProcessFrame.
// Emits one line:
//   `<seed> <width> <height> <state_hash> <rng> <level> <worm0> <worm1>
//    <bobjects> <bonuses> <sobjects> <nobjects> <wobjects>`
// (seed/width/height decimal; every hash as %08x). Links the `game` library;
// built via the OPENLIERO_BUILD_ORACLE_DUMP CMake option (see gen_sim_golden.sh).
// Not part of the default build.
//
// Why a LOADED level, not GenerateFromSettings: random generation consumes RNG
// and would move `rand.last` off 0; loading a fixed `.lev` keeps the tick-0 state
// reproducible (rand.last == 0, cycles == 0, empty pools).
//
// StartGame() is intentionally NOT called: with game_mode == kGmKillEmAll and a
// NullSoundPlayer it only plays a (null) sound, resizes the (empty) bobjects pool
// capacity, and resets the StatsRecorder — none of which alters a tick-0 hashed
// field (the bobjects pool has zero live entries, so the hash is unchanged).
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <memory>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "stateHash.hpp"
#include "weapon.hpp"
#include "worm.hpp"

namespace {

std::vector<uint8_t> SlurpFile(char const* path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    std::fprintf(stderr, "cannot open level %s\n", path);
    std::exit(1);
  }
  return std::vector<uint8_t>(std::istreambuf_iterator<char>(f),
                              std::istreambuf_iterator<char>());
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::fprintf(stderr, "usage: oracle_dump_sim <level.lev> <out.txt> [seed]\n");
    return 1;
  }
  uint32_t const seed = (argc >= 4) ? static_cast<uint32_t>(std::strtoul(argv[3], nullptr, 10)) : 42;

  PrecomputeTables();

  auto common = std::make_shared<Common>();
  FsNode const kTcRoot(FsNode("data") / "TC" / "openliero");
  common->load(kTcRoot);

  auto settings = std::make_shared<Settings>();
  settings->game_mode = Settings::kGmKillEmAll;
  settings->lives = 10;
  settings->loading_time = 0;

  auto sound_player = std::make_shared<NullSoundPlayer>();
  Game game(common, settings, sound_player);
  game.rand.Seed(seed);

  // Load a FIXED level (NOT GenerateFromSettings, which would consume RNG).
  {
    std::vector<uint8_t> const buf = SlurpFile(argv[1]);
    io::MemReader r(buf);
    if (!game.level.load(*common, *settings, r)) {
      std::fprintf(stderr, "Level::load failed for %s\n", argv[1]);
      return 1;
    }
  }

  // Add 2 worms exactly as the determinism fixture (test_determinism.cpp).
  for (int idx = 0; idx < 2; ++idx) {
    auto w = std::make_shared<Worm>();
    w->settings = settings->worm_settings[idx];
    w->health = w->settings->health;
    w->index = idx;
    w->stats_x = idx == 0 ? 0 : 218;
    game.AddWorm(w);
  }
  for (auto const& w : game.worms) {
    w->InitWeapons(game);
  }
  game.ResetWorms();

  // Tick-0 hashes — dumped BEFORE any ProcessFrame.
  uint32_t const state_hash = HashGameState(game);
  ComponentHashes const c = HashGameComponents(game);

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  std::fprintf(out, "%u %d %d %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x\n", seed,
               game.level.width, game.level.height, state_hash, c.rng, c.level, c.worms[0],
               c.worms[1], c.bobjects, c.bonuses, c.sobjects, c.nobjects, c.wobjects);
  std::fclose(out);
  return 0;
}
