// Generates the golden record for the Rust sim slice-2 *per-tick physics*
// differential test. Builds a real C++ `Game` (seed RNG, load a FIXED `.lev`, add
// 2 worms, InitWeapons, ResetWorms), places the worms mid-air per a scenario file,
// then drives them N ticks. Each tick runs a ProcessFrame *subset*: first the object
// loops (sobjects -> wobjects -> nobjects -> bobjects, in `Game::ProcessFrame` order),
// then the UNMODIFIED `worm->Process(game)` for each worm in `game.worms` order. It
// dumps one hash record per tick (tick 0 plus one after each of the N passes => N+1
// lines). Each line:
//   `<tick> <HashGameState> <rng> <level> <worm0> <worm1> <bobjects> <bonuses>
//    <sobjects> <nobjects> <wobjects>`
// (tick decimal; every hash as %08x). See stateHash.hpp for the hashes.
//
// Why a ProcessFrame *subset* and NOT the full `Game::ProcessFrame`: ProcessFrame
// draws RNG every tick (the bonus-drop roll), increments `cycles`, and runs the
// ninjarope / game-mode logic — all Slice-6 ProcessFrame-integration concerns, not
// the object+worm physics this oracle exercises. We DO run the object loops (so a
// fired projectile advances), but EXCLUDE `++cycles`, the bonus-drop roll, the
// bonuses loop, ninjarope, and the game-mode switch. Under empty input and full
// health, with no projectiles in flight, every RNG-drawing / pool-spawning branch of
// `Process` is skipped and the object pools are empty, so the loops are no-ops:
// `rand.last` stays 0 (the `rng` column is a constant 0), `cycles` stays 0, and the
// level is never dug. The dumper must NOT call ProcessFrame, ++cycles, or
// GenerateFromSettings.
//
// Why a LOADED level, not GenerateFromSettings: random generation consumes RNG and
// would move `rand.last` off 0; loading a fixed `.lev` keeps the run reproducible.
//
// Scenario file (argv[1]) — whitespace-separated, `#` comments, blank lines ok:
//   seed <u32>
//   level <path relative to data/TC/openliero>
//   ticks <N>
//   worm <idx> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>
//   input <tick> <worm0_7bit> <worm1_7bit>   (sparse; absent => 0; applied on the
//                                              Process pass advancing <tick>-><tick>+1)
//   weapon <slot> <name>   (override BOTH worms' weapon slot <slot> with the named
//                           weapon from `common->weapons`, full ammo, ready to fire)
//
// Diagnostic: set env OL_PHYS_TRACE=1 to also print per-tick pos/vel for both worms
// to stderr (does not affect the golden output). Built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option (see gen_sim_physics_golden.sh). Not part
// of the default build.
#include <array>
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
    std::fprintf(stderr, "usage: oracle_dump_sim_physics <scenario.txt> <out.txt> [seed]\n");
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
  settings->game_mode = Settings::kGmKillEmAll;
  settings->lives = scn.worms[0].lives;
  settings->loading_time = 0;
  // O4: omit CorrectShadow for the dirt-effect slices. CorrectShadow (blit.cpp:624,
  // gated on settings->shadow) writes material_id and IS reachable from this dumper's
  // Process loop (worm dig, dirt-effect / expl_ground explosions). It is inert to
  // slices 1-4a only because those scenarios trigger no such event in the dumped ticks;
  // the empty re-diff confirms that. (MakeShadow, the other shadow material_id writer,
  // runs only via GenerateFromSettings, which this load()-based dumper never calls.)
  settings->shadow = false;

  auto sound_player = std::make_shared<NullSoundPlayer>();
  Game game(common, settings, sound_player);
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
  // them AFTER it). No viewports — we never call ProcessFrame.
  for (int idx = 0; idx < 2; ++idx) {
    WormSpec const& spec = scn.worms[idx];
    auto const& w = game.worms[idx];
    w->pos = {spec.pos_x, spec.pos_y};
    w->vel = {0, 0};
    w->health = spec.health;
    w->lives = spec.lives;
    w->visible = spec.visible != 0;

    // Apply per-slot weapon overrides (ResetWorms re-ran InitWeapons, so do this
    // after it). Set the slot ready to fire: full ammo, no delay, not loading. The
    // Fire gate needs Available() (loading_left == 0) and delay_left <= 0.
    for (auto const& [slot, name] : scn.weapon_overrides) {
      WormWeapon& ww = w->weapons[slot];
      ww.type = &common->weapons[ResolveWeapon(*common, name)];
      ww.ammo = ww.type->ammo;
      ww.delay_left = 0;
      ww.loading_left = 0;
    }
    w->current_weapon = 0;
  }

  std::FILE* out = std::fopen(argv[2], "w");
  if (!out) {
    std::fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }

  bool const trace = std::getenv("OL_PHYS_TRACE") != nullptr;

  auto dump = [&](int tick) {
    uint32_t const state_hash = HashGameState(game);
    ComponentHashes const c = HashGameComponents(game);
    std::fprintf(out, "%d %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x\n", tick, state_hash,
                 c.rng, c.level, c.worms[0], c.worms[1], c.bobjects, c.bonuses, c.sobjects,
                 c.nobjects, c.wobjects);
    if (trace) {
      auto const& w0 = game.worms[0];
      auto const& w1 = game.worms[1];
      std::fprintf(stderr, "%3d  w0 pos(%d,%d) vel(%d,%d)  w1 pos(%d,%d) vel(%d,%d)\n", tick,
                   w0->pos.x, w0->pos.y, w0->vel.x, w0->vel.y, w1->pos.x, w1->pos.y, w1->vel.x,
                   w1->vel.y);
    }
  };

  // Tick 0: the proven start state, before any motion.
  dump(0);

  // Drive N ticks: apply scripted input, Process each worm in game.worms order,
  // then dump. The input for the pass advancing tick t -> t+1 is keyed on t.
  for (int t = 0; t < scn.ticks; ++t) {
    // Object loops, in `Game::ProcessFrame` order (game.cpp:333-355), BEFORE the
    // worm loop. EXCLUDES ++cycles, the bonus-drop roll, the bonuses loop, ninjarope,
    // and the game-mode switch. On the empty pools of non-firing scenarios these are
    // no-ops; once a worm Fires, the spawned projectile advances here next tick.
    {
      auto sr = game.sobjects.All();
      for (SObject* i = nullptr; (i = sr.Next());) {
        i->Process(game);
      }
      auto wr = game.wobjects.All();
      for (WObject* i = nullptr; (i = wr.Next());) {
        i->Process(game);
      }
      auto nr = game.nobjects.All();
      for (NObject* i = nullptr; (i = nr.Next());) {
        i->Process(game);
      }
      for (Game::BObjectList::Iterator i = game.bobjects.Begin(); i != game.bobjects.End();) {
        if (i->Process(game)) {
          ++i;
        } else {
          game.bobjects.Free(i);
        }
      }
    }

    std::array<uint32_t, 2> in{0, 0};
    auto it = scn.inputs.find(t);
    if (it != scn.inputs.end()) {
      in = it->second;
    }
    for (int idx = 0; idx < static_cast<int>(game.worms.size()); ++idx) {
      auto const& w = game.worms[idx];
      w->control_states.Unpack(idx < 2 ? in[idx] : 0);
      w->Process(game);
    }
    dump(t + 1);
  }

  std::fclose(out);
  return 0;
}
