// Generates the golden record for the Rust sim slice-2 *per-tick physics*
// differential test. Builds a real C++ `Game` (seed RNG, load a FIXED `.lev`, add
// 2 worms, InitWeapons, ResetWorms), places the worms mid-air per a scenario file,
// then drives them N ticks. Each tick runs the full `Game::ProcessFrame` TAIL: first
// the object loops (sobjects -> wobjects -> nobjects -> bobjects, in `Game::ProcessFrame`
// order), then the UNMODIFIED `worm->Process(game)` for each worm in `game.worms` order,
// then (Slice 6) the ninjarope `Process` loop and the game-mode switch. It dumps one hash
// record per tick (tick 0 plus one after each of the N passes => N+1 lines). Each line:
//   `<tick> <HashGameState> <rng> <level> <worm0> <worm1> <bobjects> <bonuses>
//    <sobjects> <nobjects> <wobjects>`
// (tick decimal; every hash as %08x). See stateHash.hpp for the hashes.
//
// Why the full ProcessFrame TAIL and NOT the literal `Game::ProcessFrame`: this dumper
// hand-rolls the frame so it can install a no-op StatsRecorder, run headless (no
// viewports), and load a fixed level. It reproduces every HASHED / RNG-bearing step of
// `Game::ProcessFrame` (game.cpp:267-471) in the exact C++ order: the bonuses Process
// loop (Slice 5c), the object loops (so a fired projectile advances), `++game.cycles`
// at the exact `game.cpp:357` point (after the four object loops, before the worm loop)
// so the blood-trail / animation gates that key on `cycles` are faithful (Slice 5b), the
// per-tick bonus-drop roll (`game.cpp:359-362`, GATED on `max_bonuses`, Slice 5c), the
// worm Process loop, then (Slice 6) the ninjarope `Process` loop (`game.cpp:368-370`)
// and the game-mode switch (`game.cpp:372-461`, defaulting to kGmKillEmAll =>
// `default: break`). The three render/snapshot-only steps are OMITTED as provably
// hash-inert (design §1): `--screen_flash` (in GameSnapshot, absent from HashGameState),
// the viewport/spectator `shake` decrements and the `(cycles&1)` banner-y walk (both
// iterate the empty viewport lists), and `ProcessViewports` + the `prev_control_states`
// store (`prev_control_states` is not hashed; the Rust PressedOnce model subsumes the
// edge). Under empty input, full health, max_bonuses 0 and no projectiles in flight,
// every RNG-drawing / pool-spawning branch is skipped and the object pools are empty, so
// the loops are no-ops: `rand.last` stays 0 (the `rng` column is a constant 0) and the
// level is never dug. `cycles` ADVANCES once per tick: it folds into the master
// `HashGameState` (stateHash.hpp:19) but NOT into any component hash, so it perturbs only
// the master column. The dumper must NOT call ProcessFrame or GenerateFromSettings.
//
// Why a LOADED level, not GenerateFromSettings: random generation consumes RNG and
// would move `rand.last` off 0; loading a fixed `.lev` keeps the run reproducible.
//
// Scenario file (argv[1]) — whitespace-separated, `#` comments, blank lines ok:
//   seed <u32>
//   level <path relative to data/TC/openliero>
//   ticks <N>
//   max_bonuses <n>   (Settings::max_bonuses; default 0 => the bonus-drop roll
//                      short-circuits and draws no rand; > 0 opens the roll, Slice 5c)
//   game_mode <n>     (Settings::game_mode enum; default 0 = kGmKillEmAll => the
//                      game-mode switch hits `default: break` and is inert, Slice 6)
//   worm <idx> <pos_x_fixed> <pos_y_fixed> <health> <lives> <stats_x> <visible>
//   input <tick> <worm0_7bit> <worm1_7bit>   (sparse; absent => 0; applied on the
//                                              Process pass advancing <tick>-><tick>+1)
//   weapon <slot> <name> [ammo]   (override BOTH worms' weapon slot <slot> with the
//                                  named weapon from `common->weapons`, full ammo,
//                                  ready to fire; optional 3rd token is an opt-in
//                                  low-ammo override to reach the reload branch quickly)
//
// Diagnostic: set env OL_PHYS_TRACE=1 to also print per-tick pos/vel for both worms
// to stderr (does not affect the golden output). Built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option (see gen_sim_physics_golden.sh). Not part
// of the default build.
#include <algorithm>
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
  // short-circuits (no rand drawn), so scenarios without a `max_bonuses` directive stay
  // byte-identical. A scenario sets it > 0 to make the bonus pool live (Slice 5c).
  int max_bonuses = 0;
  // Settings::game_mode for the game-mode switch (game.cpp:372-461). Default
  // kGmKillEmAll (0) => the switch hits `default: break` and is inert, so scenarios
  // without a `game_mode` directive stay byte-identical (Slice 6). A scenario sets it
  // to exercise GameOfTag / Holdazone / ScalesOfJustice.
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
  // Game-mode switch (game.cpp:372-461) is driven by the scenario's `game_mode`
  // directive; default kGmKillEmAll (0) => `default: break` => inert (Slice 6).
  settings->game_mode = scn.game_mode;
  settings->lives = scn.worms[0].lives;
  settings->loading_time = 0;
  // O4: omit CorrectShadow for the dirt-effect slices. CorrectShadow (blit.cpp:624,
  // gated on settings->shadow) writes material_id and IS reachable from this dumper's
  // Process loop (worm dig, dirt-effect / expl_ground explosions). It is inert to
  // slices 1-4a only because those scenarios trigger no such event in the dumped ticks;
  // the empty re-diff confirms that. (MakeShadow, the other shadow material_id writer,
  // runs only via GenerateFromSettings, which this load()-based dumper never calls.)
  settings->shadow = false;
  // Bonus-drop roll gate. The in-game default is 4 (settings.hpp:69); this dumper
  // overrides it from the scenario (default 0) so that scenarios WITHOUT a
  // `max_bonuses` directive leave it 0 — the per-tick roll then short-circuits
  // (`max_bonuses > 0` is false => no rand drawn), keeping their goldens byte-identical.
  // A scenario that sets `max_bonuses > 0` opens the roll (Slice 5c).
  settings->max_bonuses = scn.max_bonuses;

  auto sound_player = std::make_shared<NullSoundPlayer>();
  Game game(common, settings, sound_player);
  // O15: install the base no-op StatsRecorder. A default-constructed Game has a null
  // `stats_recorder`; once Slice 5b's worm-damage path runs, `DamageDealt` would
  // dereference it and crash headless. The base `StatsRecorder` (stats_recorder.cpp:8-29)
  // is a pure no-op (the crashing subclass is `NormalStatsRecorder`, :44-77), so this is
  // inert for slices 1-5a — they never hit a worm.
  game.stats_recorder = std::make_shared<StatsRecorder>();
  // StartGame parity for the blood pool: the real game sizes `bobjects` from
  // `settings->blood_particle_max` in `Game::StartGame` (game.cpp:513). This dumper
  // hand-rolls setup and never calls `StartGame` (it would play sounds / reset stats /
  // spawn zones), so without this the `BObjectList` keeps its `FastObjectList` default
  // limit of 1 — and `NewObjectReuse` then OVERWRITES that single slot on every
  // `CreateBObject`, collapsing a whole blood spray to ONE particle (a dumper artifact,
  // not real game behaviour). Resizing here mirrors `StartGame` so the golden's
  // `bobjects` column reflects the true 700-cap pool the Rust `SimState` models. Inert
  // for slices 1-5a (they spawn no bobjects). Uses no RNG.
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
      // Opt-in low-ammo override (only when the `weapon` directive had a 3rd token).
      auto ammo_it = scn.weapon_ammo_overrides.find(slot);
      if (ammo_it != scn.weapon_ammo_overrides.end()) {
        ww.ammo = ammo_it->second;
      }
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
    // Bonuses Process loop (game.cpp:287-290), at the TOP of ProcessFrame, BEFORE
    // the object loops AND before `++cycles`. `bonuses` is an ExactObjectList (slot
    // order; All() skips free slots); `Bonus::Process` (fall/bounce/expire) may
    // Free(this) on the expire path. For non-5c scenarios the pool is empty ⇒ a
    // no-op ⇒ byte-identical. Made live by a `max_bonuses > 0` scenario (Slice 5c).
    {
      auto br = game.bonuses.All();
      for (Bonus* i = nullptr; (i = br.Next());) {
        i->Process(game);
      }
    }

    // Object loops, in `Game::ProcessFrame` order (game.cpp:333-355), BEFORE the
    // worm loop. EXCLUDES the bonus-drop roll, ninjarope, and the game-mode switch
    // (still Slice-6 concerns). On the empty pools of non-firing scenarios these are
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

    // O17: `++cycles` at the exact `game.cpp:357` point — AFTER the four object loops
    // (sobjects -> wobjects -> nobjects -> bobjects) and BEFORE the worm loop. The
    // object loops above ran with the value left by the previous tick (cycles=k-1 on
    // tick k); the worm loop below sees the post-increment cycles=k. It folds into the
    // master `HashGameState` only (not the components). The Rust `process_frame` must
    // increment at this SAME point — the off-by-one is load-bearing for the
    // `cycles % delay` / blood-trail gates read DURING the object loop.
    ++game.cycles;

    // Bonus-drop roll (game.cpp:359-362), AFTER `++cycles` and BEFORE the worm loop —
    // the exact game.cpp:359 point. Gated on `settings->max_bonuses > 0`: the `&&`
    // short-circuits left-to-right, so `max_bonuses == 0` (the default for scenarios
    // with no `max_bonuses` directive) draws NO rand, keeping those goldens
    // byte-identical. `h[HBonusDisable]` is false in the openliero TC. When the gate
    // opens, `rand(c[CBonusDropChance])` is drawn from the SAME RNG at this load-bearing
    // position; a 0 roll calls `Game::CreateBonus` (real C++ — the bonus then sits in
    // the pool until the bonuses Process loop, a later slice).
    if (!common->h[HBonusDisable] && settings->max_bonuses > 0 &&
        game.rand(common->c[CBonusDropChance]) == 0) {
      game.CreateBonus();
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

    // Ninjarope Process loop (game.cpp:368-370), AFTER the worm loop and BEFORE the
    // game-mode switch. The rope's `out` + `pos.{x,y}` are hashed (stateHash.hpp:47-49);
    // a rope that is not out is inert (no motion, no RNG). On a dirt attach it draws
    // `rand(128)` x11 and spawns 11 nobjects at the exact RNG position (ninjarope.cpp:
    // 41-45) — so a rope-throwing scenario diverges from its first rope-out tick (Slice
    // 6). Unmodified `Ninjarope::Process`, same as the C++ frame.
    for (auto const& w : game.worms) {
      w->ninjarope.Process(*w, game);
    }

    // Game-mode switch (game.cpp:372-461), AFTER the ninjarope loop. Verbatim from
    // `Game::ProcessFrame` with `game.`-qualified members. Only hashed effects matter:
    // GameOfTag / Holdazone `++timer`, Holdazone `SpawnZone` RNG. The scenario default
    // kGmKillEmAll hits `default: break` => inert, so KillEmAll goldens are unaffected
    // by this addition (Slice 6). The three render-only ProcessFrame steps that follow
    // in C++ (`ProcessViewports`, the `prev_control_states` store) stay OMITTED as
    // provably hash-inert (design §1).
    switch (settings->game_mode) {
      case Settings::kGmGameOfTag: {
        bool some_invisible = false;
        for (auto& worm : game.worms) {
          if (!worm->visible) {
            some_invisible = true;
            break;
          }
        }

        Worm* last_killed_by = game.WormByIdx(game.last_killed_idx);

        if (!some_invisible && last_killed_by && (game.cycles % 70) == 0 &&
            last_killed_by->timer < settings->time_to_lose) {
          ++last_killed_by->timer;
        }
      } break;

      case Settings::kGmHoldazone: {
        // Alias `game.holdazone` so the body below is a verbatim copy of
        // `Game::ProcessFrame` (game.cpp:390-458) — same line breaks / NOLINT pragmas.
        auto& holdazone = game.holdazone;
        int contender_idx = -1;
        int contenders = 0;

        for (auto const& w : game.worms) {
          int const kX = Ftoi(w->pos.x);
          int const kY = Ftoi(w->pos.y);

          if (w->visible && holdazone.rect.Inside(kX, kY)) {
            contender_idx = w->index;
            ++contenders;
          }
        }

        if (contenders == 0) {
          contender_idx = holdazone.holder_idx;
        }

        if (contenders <= 1) {
          if (contender_idx < 0 ||
              (holdazone.contender_idx != contender_idx && holdazone.contender_frames != 0)) {
            // NOLINTNEXTLINE(bugprone-inc-dec-in-conditions) — short-circuit-then-mutate is the entire point: only decrement when not already 0.
            if (holdazone.contender_frames == 0 || --holdazone.contender_frames == 0) {
              holdazone.contender_idx = contender_idx;
              holdazone.holder_idx = -1;
            }
          } else {
            holdazone.contender_idx = contender_idx;

            // NOLINTBEGIN(bugprone-inc-dec-in-conditions) — guarded increment: only fire on the exact frame that crosses the capture threshold.
            if (holdazone.contender_frames < Settings::kZoneCaptureTime &&
                ++holdazone.contender_frames >= Settings::kZoneCaptureTime &&
                holdazone.holder_idx != holdazone.contender_idx) {
              // NOLINTEND(bugprone-inc-dec-in-conditions) New holder

              int new_timeout = holdazone.timeout_left;
              if (holdazone.contender_idx >= 0) {
                new_timeout += settings->zone_timeout * 70 / 4;
              } else {
                new_timeout += settings->zone_timeout * 70 / 8;
              }

              holdazone.timeout_left = std::min(new_timeout, settings->zone_timeout * 70);

              holdazone.holder_idx = holdazone.contender_idx;
            }
          }
        }

        bool dec = false;

        if (holdazone.holder_idx >= 0) {
          auto* holder = game.WormByIdx(holdazone.holder_idx);

          if ((game.cycles % 70) == 0) {
            ++holder->timer;
          }

          dec = true;
        } else {
          dec = (game.cycles % 4) == 0;
        }

        if (dec) {
          if (--holdazone.timeout_left <= 0) {
            game.SpawnZone();
          }
        }
      } break;
      default:
        break;
    }
    dump(t + 1);
  }

  std::fclose(out);
  return 0;
}
