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
//   render <layout>               (Slice 3a; only `player` — enables the render sidecar)
//   render_shadow                 (Slice 3b; draw-time settings->shadow flip, draw window)
//   render_shake <tick> <vp> <amount>  (Slice 3b; draw-time viewports[vp]->shake inject)
//   render_flash <tick> <amount>       (Slice 3b; draw-time LightUp screen_flash inject)
//   render_hud                         (Slice 3e; draw-time HUD pre-block + minimap draw)
//
// Diagnostic: set env OL_PHYS_TRACE=1 to also print per-tick pos/vel for both worms
// to stderr (does not affect the golden output). Built via the
// OPENLIERO_BUILD_ORACLE_DUMP CMake option (see gen_sim_physics_golden.sh). Not part
// of the default build.
//
// Opt-in render sidecar (Slice 3a base + Slice 3b full world). A scenario MAY add:
//   render <layout>          (only `player` is accepted)
//   render_shadow            (draw-time only: flip settings->shadow for the draw window)
//   render_shake <tick> <vp> <amount>   (draw-time only: inject viewports[vp]->shake)
//   render_flash <tick> <amount>        (draw-time only: inject the LightUp screen_flash)
//   render_hud               (Slice 3e; draw-time only: also draw the HUD pre-block +
//                             minimap — see render_and_hash. Absent => world-only.)
// When `render` is PRESENT, the dumper additionally builds a headless `Renderer` + two
// framehash-layout `Viewport`s and, right after each `dump(tick)`, renders the FULL WORLD
// frame (the world subset of `Game::Draw`, game.cpp:170-198, + the two-pass
// `Viewport::Draw` world block, viewport.cpp:196-590) — palette rebuild (with the injected
// screen_flash -> LightUp) -> Fill(0) -> per viewport Process -> clip -> DrawLevel ->
// shadow pass (if settings->shadow) -> sprite pass -> restore clip. This mirrors the Rust
// `frame::draw` 1:1. When `render_hud` is present (Slice 3e) it ALSO draws, per viewport,
// the HUD pre-block (into the full-surface clip, before the world clip) and the minimap
// (after the world block) — see render_and_hash. When `render_hud` is absent the HUD /
// minimap are OMITTED (world-only, byte-identical to 3a/3b); name-labels/holdazone/
// banners/AI-debug stay omitted regardless. It writes a sidecar frame golden to argv[4]: one
// `<tick> <frame_hash_hex16> <state_hash_hex8>` line per tick plus a final `total <n>
// <acc_hex16>`. The frame hash is FNV-1a over the ARGB back buffer with the composition
// fade (0 on tick 0 => black, 33 => identity after), exactly as framehash_main.cpp.
//
// When the `render` directive is ABSENT (every existing sim scenario) the render block is
// fully gated off: no `Renderer` is constructed, nothing is drawn, no sidecar is opened,
// and the argv[2] sim output is BYTE-IDENTICAL to before. The render path reads only the
// level/objects/worms/cycles and the PER-VIEWPORT RNG (laser sight + shake); it never
// touches `game.rand` and never mutates sim state. The three opt-in draw-time injections
// are provably sim-neutral: `render_shadow` flips settings->shadow AFTER the tick's sim
// Process already ran (so it never reaches CorrectShadow) and restores it before the next
// Process; `render_shake` sets/restores viewports[vp]->shake around a single Process (the
// reduced dumper's viewports are never wired into ProcessViewports, so the sim never reads
// it); `render_flash` feeds only the palette LightUp (screen_flash lives in GameSnapshot,
// absent from HashGameState). Hence the re-diff gate over the existing sim goldens stays
// empty, and the terrain-only render_slice3a golden regenerates byte-identical (invisible
// worms + empty pools => the full world block paints exactly the terrain-only frame).
#include <algorithm>
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
#include "filesystem.hpp"
#include "game.hpp"
#include "constants.hpp"
#include "gfx/blit.hpp"
#include "gfx/renderer.hpp"
#include "gfx/shadow_query.hpp"
#include "io/stream.hpp"
#include "level.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "stateHash.hpp"
#include "stats_recorder.hpp"
#include "text.hpp"
#include "viewport.hpp"
#include "weapon.hpp"
#include "worm.hpp"

namespace {

// FNV-1a frame-hash constants + the composition fade, copied verbatim from
// framehash_main.cpp:26-34 so the sidecar frame hash matches the reference hasher.
constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;

uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }

uint8_t FadeChannel(uint8_t v, int amount) {
  return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
}

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
  // Opt-in render layout (Slice 3a). Empty => the render path is fully off and the
  // sim output stays byte-identical. Only `player` is accepted (two-viewport 320x200).
  std::string render_layout;
  // Opt-in draw-time shadow flip (Slice 3b, spec O2). When true, render_and_hash
  // temporarily sets `settings->shadow = true` for the DRAW window only (restored
  // immediately after, AFTER the tick's sim Process already ran — so the flip never
  // reaches CorrectShadow, and the sim output stays byte-identical). Absent => the
  // draw-time shadow pass is gated off exactly as before.
  bool render_shadow = false;
  // Opt-in draw-only screen-shake injection (Slice 3b, spec O4). tick -> {vp -> amount}.
  // render_and_hash sets `viewports[vp]->shake = Itof(amount)` BEFORE that viewport's
  // Process for the matching tick (so the shake branch, viewport.cpp:49-52, draws its
  // two rand off the viewport RNG), then restores it. Sim untouched — the reduced
  // dumper's viewports are never wired into ProcessViewports.
  std::map<int, std::map<int, int>> render_shake;
  // Opt-in draw-only screen-flash injection (Slice 3b, spec O4). tick -> amount.
  // Passed as the `LightUp` argument into the per-tick palette build (game.cpp:179-181)
  // for the matching tick only. Sim untouched (screen_flash lives in GameSnapshot,
  // absent from HashGameState).
  std::map<int, int> render_flash;
  // Opt-in HUD/minimap draw (Slice 3e). When true, render_and_hash additionally
  // draws the HUD pre-block (viewport.cpp:84-189, KillEmAll/Scales arms,
  // is_replay=false) into the full-surface clip BEFORE the per-viewport world
  // clip, and the 52x36 minimap + worm dots (viewport.cpp:593-613) AFTER the
  // world block, gated on `settings->map`. 1:1 with the Rust `frame::draw` HUD
  // path (render/src/frame.rs, render/src/hud.rs). Sim-neutral: the draw-window
  // `settings->map` flip never reaches a sim mutation. Absent (every existing
  // scenario) => the block is skipped and the world-only draw stays
  // byte-identical (the re-diff gate).
  bool render_hud = false;
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
    } else if (key == "render") {
      ls >> s.render_layout;
      if (s.render_layout != "player") {
        std::fprintf(stderr, "unknown render layout: %s\n", s.render_layout.c_str());
        std::exit(1);
      }
    } else if (key == "render_shadow") {
      // Draw-time shadow flip (Slice 3b). 0 args — presence flips the draw window.
      s.render_shadow = true;
    } else if (key == "render_shake") {
      // render_shake <tick> <vp> <amount> — draw-only shake injection (Slice 3b).
      int tick = 0;
      int vp = 0;
      int amount = 0;
      ls >> tick >> vp >> amount;
      s.render_shake[tick][vp] = amount;
    } else if (key == "render_flash") {
      // render_flash <tick> <amount> — draw-only screen-flash injection (Slice 3b).
      int tick = 0;
      int amount = 0;
      ls >> tick >> amount;
      s.render_flash[tick] = amount;
    } else if (key == "render_hud") {
      // Draw-time HUD/minimap draw (Slice 3e). 0 args — presence enables it.
      s.render_hud = true;
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
    std::fprintf(stderr,
                 "usage: oracle_dump_sim_physics <scenario.txt> <out.txt> [seed] [frames.txt]\n");
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

  // ---- Opt-in render (Slice 3a). Absent render_layout => nothing below runs and the
  // sim output stays byte-identical (the re-diff gate). The sidecar path is argv[4]. ----
  std::unique_ptr<Renderer> renderer;
  std::vector<std::unique_ptr<Viewport>> viewports;
  std::FILE* frames = nullptr;
  uint64_t frames_acc = kFnvOffset;  // FNV offset seed (framehash_main.cpp:110).
  int frame_count = 0;
  if (!scn.render_layout.empty()) {
    if (argc < 5) {
      std::fprintf(stderr, "render scenario needs a 4th arg (frames sidecar path)\n");
      return 1;
    }
    frames = std::fopen(argv[4], "w");
    if (!frames) {
      std::fprintf(stderr, "cannot open %s\n", argv[4]);
      return 1;
    }
    renderer = std::make_unique<Renderer>();
    renderer->Init(320, 200);
    renderer->LoadPalette(*common);  // Origpal = common.exepal (Classic).
    // Two viewports, framehash layout (framehash_main.cpp:92-95).
    viewports.push_back(std::make_unique<Viewport>(Rect(0, 0, 158, 158), game.worms[0]->index));
    viewports.push_back(
        std::make_unique<Viewport>(Rect(160, 0, 158 + 160, 158), game.worms[1]->index));
  }

  // Renders the FULL world block (the world subset of Game::Draw, game.cpp:170-198, plus
  // the two-pass Viewport::Draw world block, viewport.cpp:196-590), hashes it, and appends
  // one sidecar line. This is the C++ mirror of the Rust `frame::draw` (render/src/frame.rs):
  // palette build (with the injected screen_flash -> LightUp) -> Fill(0) -> per viewport
  // Process -> clip -> DrawLevel at kOffs -> shadow pass (if settings->shadow) -> sprite
  // pass -> restore clip. When `render_hud` is set (Slice 3e) the HUD pre-block + minimap
  // are ALSO drawn per viewport (before/after the world block); otherwise HUD/minimap are
  // omitted (world-only). name-labels/holdazone/banners/AI-debug stay omitted. The path reads
  // only level/objects/worms/cycles and the per-viewport RNG (laser sight + shake); it
  // never touches game.rand or mutates sim state, so the sim output is untouched (design
  // §6). The render_shadow flip window and the render_shake/render_flash injections carry
  // no sim mutation (spec O2/O4).
  auto render_and_hash = [&](int tick) {
    // `common` reference so the viewport.cpp world block below (and the LC(...) macro,
    // constants.hpp:191 = common.c[C##name]) reads verbatim; shadows the captured
    // shared_ptr for the lambda body only.
    Common& common = *game.common;

    // Draw-time shadow flip (spec O2): the tick's sim Process already ran, so this never
    // reaches CorrectShadow. Restored below, before the next tick's sim Process.
    bool const saved_shadow = game.settings->shadow;
    if (scn.render_shadow) game.settings->shadow = true;

    // Draw-time map flip (Slice 3e): the HUD path draws the minimap only when
    // settings->map is set. Flip it for the DRAW WINDOW only (restored below,
    // before the next tick's sim Process), so — like the shadow flip — it never
    // reaches a sim mutation and every sim golden stays byte-identical.
    bool const saved_map = game.settings->map;
    if (scn.render_hud) game.settings->map = true;

    // Draw-only screen-flash injection (spec O4). Absent for this tick => 0 => no LightUp.
    int screen_flash = 0;
    {
      auto const it = scn.render_flash.find(tick);
      if (it != scn.render_flash.end()) screen_flash = it->second;
    }

    // Palette build order (game.cpp:171-183): reset -> RotateFrom -> LightUp (if flash) ->
    // UpdatePal32. Mirrors the Rust build_palette(.., screen_flash).
    renderer->pal = renderer->Origpal();
    for (auto const& w : common.color_anim) {
      renderer->pal.RotateFrom(renderer->Origpal(), w.from, w.to, game.cycles >> 3);
    }
    if (screen_flash > 0) {
      renderer->pal.LightUp(screen_flash);  // game.cpp:179-181
    }
    renderer->UpdatePal32();
    Fill(renderer->bmp, 0);  // game.cpp:189

    for (std::size_t vi = 0; vi < viewports.size(); ++vi) {
      auto const& vp = viewports[vi];

      // Draw-only shake injection (spec O4). shake is `fixed`, so inject Itof(amount) and
      // the shake branch (viewport.cpp:47-52) recovers the pixel amount via Ftoi. Set
      // BEFORE Process (which reads it), restored immediately after so the next viewport /
      // tick starts clean and the sim never sees it.
      fixed const saved_shake = vp->shake;
      {
        auto const tit = scn.render_shake.find(tick);
        if (tit != scn.render_shake.end()) {
          auto const vit = tit->second.find(static_cast<int>(vi));
          if (vit != tit->second.end()) vp->shake = Itof(vit->second);
        }
      }
      vp->Process(game);
      vp->shake = saved_shake;

      // ---- HUD pre-block (Slice 3e), gated on render_hud. Ported from
      //      viewport.cpp:84-189 (KillEmAll/Scales arms; is_replay=false, so the
      //      replay block :134-144 is skipped). Drawn into the FULL-SURFACE clip
      //      BEFORE the per-viewport world clip is set — 1:1 with the Rust
      //      frame::draw HUD path (render/src/frame.rs:80-91, render/src/hud.rs).
      //      Reads only worm/cycles/settings; no sim mutation. ----
      if (scn.render_hud) {
        renderer->bmp.clip_rect = Rect(0, 0, renderer->bmp.w, renderer->bmp.h);
        int const kMultiplier = renderer->bmp.w / 320;  // viewport.cpp:81
        int const kRenderResY = renderer->bmp.h;        // renderer.render_res_y
        Worm const& hud_worm = *game.WormByIdx(vp->worm_idx);
        int const kStatsX = hud_worm.stats_x * kMultiplier;  // viewport.cpp:86

        if (hud_worm.visible) {  // viewport.cpp:84-87
          int const kLifebarWidth = hud_worm.health * 100 / hud_worm.settings->health;
          DrawBar(renderer->bmp, kStatsX, kRenderResY - 39, kLifebarWidth,
                  kLifebarWidth / 10 + 234);
        } else {  // viewport.cpp:88-95
          int lifebar_width = 100 - (hud_worm.killed_timer * 25) / 37;
          if (lifebar_width > 0) {
            lifebar_width = std::min(lifebar_width, 100);
            DrawBar(renderer->bmp, kStatsX, kRenderResY - 39, lifebar_width,
                    lifebar_width / 10 + 234);
          }
        }

        WormWeapon const& ww = hud_worm.weapons[hud_worm.current_weapon];  // viewport.cpp:99
        if (ww.Available()) {                                              // viewport.cpp:101
          if (ww.ammo > 0) {                                               // viewport.cpp:102
            int const kAmmoBarWidth = ww.ammo * 100 / ww.type->ammo;       // viewport.cpp:103
            if (kAmmoBarWidth > 0) {
              DrawBar(renderer->bmp, kStatsX, kRenderResY - 34, kAmmoBarWidth,
                      kAmmoBarWidth / 10 + 245);  // viewport.cpp:106-107
            }
          }
        } else {  // viewport.cpp:110-129
          int ammo_bar_width = 0;
          if (ww.type->loading_time != 0) {  // viewport.cpp:113-115
            int const kComputedLoadingTime = ww.type->ComputedLoadingTime(*game.settings);
            ammo_bar_width = 100 - ww.loading_left * 100 / kComputedLoadingTime;
          } else {  // viewport.cpp:117
            ammo_bar_width = 100 - ww.loading_left * 100;
          }
          if (ammo_bar_width > 0) {
            DrawBar(renderer->bmp, kStatsX, kRenderResY - 34, ammo_bar_width,
                    ammo_bar_width / 10 + 245);  // viewport.cpp:121-122
          }
          if ((game.cycles % 20) > 10 && hud_worm.visible) {  // viewport.cpp:125-128
            common.font.DrawString(renderer->bmp, LS(Reloading), kStatsX, 164 * kMultiplier, 50);
          }
        }

        common.font.DrawString(renderer->bmp, (LS(Kills) + ToString(hud_worm.kills)), kStatsX,
                               kRenderResY - 29, 10);  // viewport.cpp:131-132

        // is_replay == false => the replay HUD (viewport.cpp:134-144) is skipped.

        switch (game.settings->game_mode) {  // viewport.cpp:148-189
          case Settings::kGmKillEmAll:
          case Settings::kGmScalesOfJustice: {
            common.font.DrawString(renderer->bmp, (LS(Lives) + ToString(hud_worm.lives)), kStatsX,
                                   kRenderResY - 22, 6);  // viewport.cpp:151-152
          } break;
          default:
            break;  // Holdazone/GameOfTag deferred (our render scenarios are KillEmAll).
        }
      }

      renderer->bmp.clip_rect = vp->rect;
      renderer->bmp.cycles = game.cycles;
      fixedvec const kOffs = vp->rect.Ul() - IVec2(vp->x, vp->y);  // viewport.cpp:198

      // Shadows and explosion masks query the level (screen + offset = world). Same
      // world_offset = -kOffs convention as the Rust frame::draw/shadow_pass.
      ShadowQuery const kShadow{.common = common,
                                .level = game.level,
                                .pal32 = renderer->pal32,
                                .world_offset_x = -kOffs.x,
                                .world_offset_y = -kOffs.y,
                                .mode = renderer->mode,
                                .cycles = game.cycles};

      DrawLevel(renderer->bmp, game.level, kOffs.x, kOffs.y);  // viewport.cpp:210

      Worm const& vp_worm = *game.WormByIdx(vp->worm_idx);

      // ---- Pass 1: all shadows (viewport.cpp:274-398), gated on settings->shadow. ----
      if (game.settings->shadow) {
        {  // bonuses — viewport.cpp:276-284
          auto br = game.bonuses.All();
          for (Bonus const* i = nullptr; (i = br.Next());) {
            if (i->timer > LC(BonusFlickerTime) || (game.cycles & 3) == 0) {
              int const kF = common.bonus_frames[i->frame];
              BlitShadowImage(kShadow, renderer->bmp, common.small_sprites.SpritePtr(kF),
                              Ftoi(i->x) - 5 + kOffs.x, Ftoi(i->y) - 1 + kOffs.y, 7, 7);
            }
          }
        }

        {  // sobjects — viewport.cpp:287-298
          auto sr = game.sobjects.All();
          for (SObject const* i = nullptr; (i = sr.Next());) {
            SObjectType const& t = common.sobject_types[i->id];
            int const kFrame = i->cur_frame + t.start_frame;
            BlitShadowImage(kShadow, renderer->bmp, common.large_sprites.SpritePtr(kFrame),
                            i->x + kOffs.x - 3, i->y + kOffs.y + 3, 16, 16);
          }
        }

        {  // wobjects — viewport.cpp:300-343
          auto wr = game.wobjects.All();
          for (WObject const* i = nullptr; (i = wr.Next());) {
            Weapon const& w = *i->type;
            if (w.start_frame > -1) {
              int cur_frame = i->cur_frame;
              int const kShotType = w.shot_type;
              if (kShotType == 2) {
                cur_frame += 4;
                cur_frame >>= 3;
                if (cur_frame < 0) {
                  cur_frame = 16;
                } else if (cur_frame > 15) {
                  cur_frame -= 16;
                }
              } else if (kShotType == 3) {
                if (cur_frame > 64) {
                  --cur_frame;
                }
                cur_frame -= 12;
                cur_frame >>= 3;
                if (cur_frame < 0) {
                  cur_frame = 0;
                } else if (cur_frame > 12) {
                  cur_frame = 12;
                }
              }
              int const kPosX = Ftoi(i->pos.x) - 3;
              int const kPosY = Ftoi(i->pos.y) - 3;
              if (w.shadow) {
                BlitShadowImage(kShadow, renderer->bmp,
                                common.small_sprites.SpritePtr(w.start_frame + cur_frame),
                                kPosX - 3 + kOffs.x, kPosY + 3 + kOffs.y, 7, 7);
              }
            } else if (i->cur_frame > 0) {
              int const kPosX = Ftoi(i->pos.x) + kOffs.x - 3;
              int const kPosY = Ftoi(i->pos.y) + kOffs.y + 3;
              uint32_t const kShadowed = kShadow.ShadowedArgb(kPosX, kPosY);
              if (kShadowed != 0 && renderer->bmp.clip_rect.Inside(kPosX, kPosY)) {
                renderer->bmp.GetPixel(kPosX, kPosY) = kShadowed;
              }
            }
          }
        }

        {  // nobjects — viewport.cpp:345-366
          auto nr = game.nobjects.All();
          for (NObject const* i = nullptr; (i = nr.Next());) {
            NObjectType const& t = *i->type;
            if (t.start_frame > 0) {
              auto pos = Ftoi(i->pos) - IVec2(3, 3);
              BlitShadowImage(kShadow, renderer->bmp,
                              common.small_sprites.SpritePtr(t.start_frame + i->cur_frame),
                              pos.x - 3 + kOffs.x, pos.y + 3 + kOffs.y, 7, 7);
            } else if (i->cur_frame > 1) {
              auto pos = Ftoi(i->pos) + kOffs;
              pos.x -= 3;
              pos.y += 3;
              if (renderer->bmp.clip_rect.Encloses(pos)) {
                uint32_t const kShadowed = kShadow.ShadowedArgb(pos.x, pos.y);
                if (kShadowed != 0) {
                  renderer->bmp.GetPixel(pos.x, pos.y) = kShadowed;
                }
              }
            }
          }
        }

        for (auto const& worm_ptr : game.worms) {  // worms + ninjarope — viewport.cpp:368-385
          Worm const& w = *worm_ptr;
          if (w.visible) {
            int const kTempX = Ftoi(w.pos.x) - 7 + kOffs.x;
            int const kTempY = Ftoi(w.pos.y) - 5 + kOffs.y;
            if (w.ninjarope.out) {
              int const kNinjaropeX = Ftoi(w.ninjarope.pos.x) + kOffs.x;
              int const kNinjaropeY = Ftoi(w.ninjarope.pos.y) + kOffs.y;
              DrawShadowLine(kShadow, renderer->bmp, kNinjaropeX - 3, kNinjaropeY + 3,
                             kTempX + 7 - 3, kTempY + 4 + 3);
              BlitShadowImage(kShadow, renderer->bmp, common.large_sprites.SpritePtr(84),
                              kNinjaropeX - 4, kNinjaropeY + 2, 16, 16);
            }
            BlitShadowImage(kShadow, renderer->bmp,
                            common.WormSprite(w.current_frame, w.direction, w.index), kTempX - 3,
                            kTempY + 3, 16, 16);
          }
        }

        // bobjects (blood) — viewport.cpp:387-397
        for (Game::BObjectList::Iterator i = game.bobjects.Begin(); i != game.bobjects.End(); ++i) {
          auto ipos = Ftoi(i->pos) + kOffs;
          ipos.x -= 3;
          ipos.y += 3;
          if (renderer->bmp.clip_rect.Encloses(ipos)) {
            uint32_t const kShadowed = kShadow.ShadowedArgb(ipos.x, ipos.y);
            if (kShadowed != 0) {
              renderer->bmp.GetPixel(ipos.x, ipos.y) = kShadowed;
            }
          }
        }
      }

      // ---- Pass 2: all sprites (viewport.cpp:400-590). Name labels (:411/:465-479/:575-582)
      //      and AI debug (:549-551) are omitted (3e / skip). ----
      {  // bonuses — viewport.cpp:402-415
        auto br = game.bonuses.All();
        for (Bonus const* i = nullptr; (i = br.Next());) {
          if (i->timer > LC(BonusFlickerTime) || (game.cycles & 3) == 0) {
            int const kF = common.bonus_frames[i->frame];
            BlitImage(renderer->bmp, common.small_sprites[kF], Ftoi(i->x) - 3 + kOffs.x,
                      Ftoi(i->y) - 3 + kOffs.y);
          }
        }
      }

      {  // sobjects — viewport.cpp:418-425 (BlitImageR gates on the water range)
        auto sr = game.sobjects.All();
        for (SObject const* i = nullptr; (i = sr.Next());) {
          SObjectType const& t = common.sobject_types[i->id];
          int const kFrame = i->cur_frame + t.start_frame;
          BlitImageR(kShadow, renderer->bmp, common.large_sprites.SpritePtr(kFrame),
                     i->x + kOffs.x, i->y + kOffs.y, 16, 16);
        }
      }

      {  // wobjects — viewport.cpp:428-481 (sprite blit UNCONDITIONAL on w.shadow)
        auto wr = game.wobjects.All();
        for (WObject* i = nullptr; (i = wr.Next());) {
          Weapon const& w = *i->type;
          if (w.start_frame > -1) {
            int cur_frame = i->cur_frame;
            int const kShotType = w.shot_type;
            if (kShotType == 2) {
              cur_frame += 4;
              cur_frame >>= 3;
              if (cur_frame < 0) {
                cur_frame = 16;
              } else if (cur_frame > 15) {
                cur_frame -= 16;
              }
            } else if (kShotType == 3) {
              if (cur_frame > 64) {
                --cur_frame;
              }
              cur_frame -= 12;
              cur_frame >>= 3;
              if (cur_frame < 0) {
                cur_frame = 0;
              } else if (cur_frame > 12) {
                cur_frame = 12;
              }
            }
            int const kPosX = Ftoi(i->pos.x) - 3;
            int const kPosY = Ftoi(i->pos.y) - 3;
            BlitImage(renderer->bmp, common.small_sprites[w.start_frame + cur_frame],
                      kPosX + kOffs.x, kPosY + kOffs.y);
          } else if (i->cur_frame > 0) {
            int const kPosX = Ftoi(i->pos.x) + kOffs.x;
            int const kPosY = Ftoi(i->pos.y) + kOffs.y;
            renderer->bmp.SetPixel(kPosX, kPosY, static_cast<PalIdx>(i->cur_frame));
          }
        }
      }

      {  // nobjects — viewport.cpp:483-498
        auto nr = game.nobjects.All();
        for (NObject const* i = nullptr; (i = nr.Next());) {
          NObjectType const& t = *i->type;
          if (t.start_frame > 0) {
            auto pos = Ftoi(i->pos) - IVec2(3, 3);
            BlitImage(renderer->bmp, common.small_sprites[t.start_frame + i->cur_frame],
                      pos.x + kOffs.x, pos.y + kOffs.y);
          } else if (i->cur_frame > 1) {
            auto pos = Ftoi(i->pos) + kOffs;
            if (renderer->bmp.clip_rect.Encloses(pos)) {
              renderer->bmp.SetPixel(pos.x, pos.y, static_cast<PalIdx>(i->cur_frame));
            }
          }
        }
      }

      // worms — viewport.cpp:500-552. game.worms order; laser sight -> laser beam ->
      // ninjarope -> fire cone -> worm body. The laser sight advances THIS viewport's RNG.
      for (std::size_t i = 0; i < game.worms.size(); ++i) {
        Worm const& w = *game.worms[i];

        if (w.visible) {
          int const kTempX = Ftoi(w.pos.x) - 7 + kOffs.x;
          int const kTempY = Ftoi(w.pos.y) - 5 + kOffs.y;
          int const kAngleFrame = w.AngleFrame();

          if (w.weapons[w.current_weapon].Available()) {
            int const kHotspotX = w.hotspot_x + kOffs.x;
            int const kHotspotY = w.hotspot_y + kOffs.y;

            WormWeapon const& ww = w.weapons[w.current_weapon];
            Weapon const& weapon = *ww.type;

            if (weapon.laser_sight) {
              DrawLaserSight(renderer->bmp, vp->rand, kHotspotX, kHotspotY, kTempX + 7,
                             kTempY + 4);
            }

            if (ww.type - common.weapons.data() == LC(LaserWeapon) - 1 && w.Pressed(Worm::kFire)) {
              DrawLine(renderer->bmp, kHotspotX, kHotspotY, kTempX + 7, kTempY + 4,
                       weapon.color_bullets);
            }
          }

          if (w.ninjarope.out) {
            int const kNinjaropeX = Ftoi(w.ninjarope.pos.x) + kOffs.x;
            int const kNinjaropeY = Ftoi(w.ninjarope.pos.y) + kOffs.y;

            DrawNinjarope(common, renderer->bmp, kNinjaropeX, kNinjaropeY, kTempX + 7, kTempY + 4);

            BlitImage(renderer->bmp, common.large_sprites[84], kNinjaropeX - 1, kNinjaropeY - 1);
          }

          if (w.weapons[w.current_weapon].type->fire_cone > 0 && w.fire_cone > 0) {
            BlitFireCone(renderer->bmp, w.fire_cone / 2,
                         common.FireConeSprite(kAngleFrame, w.direction),
                         Common::fire_cone_offset[w.direction][kAngleFrame][0] + kTempX,
                         Common::fire_cone_offset[w.direction][kAngleFrame][1] + kTempY);
          }

          BlitImage(renderer->bmp, common.WormSpriteObj(w.current_frame, w.direction, w.index),
                    kTempX, kTempY);
        }
      }

      // aim crosshair — viewport.cpp:566-583. Gated on the VIEWPORT'S OWN worm being visible.
      if (vp_worm.visible) {
        auto temp = Ftoi(vp_worm.pos) - IVec2(1, 2) +
                    Ftoi(cossin_table[Ftoi(vp_worm.aiming_angle)] * 16) + kOffs;
        BlitImage(renderer->bmp, common.small_sprites[vp_worm.make_sight_green ? 44 : 43], temp.x,
                  temp.y);
      }

      // bobjects (blood) — viewport.cpp:585-590
      for (Game::BObjectList::Iterator i = game.bobjects.Begin(); i != game.bobjects.End(); ++i) {
        auto ipos = Ftoi(i->pos) + kOffs;
        if (renderer->bmp.clip_rect.Encloses(ipos)) {
          renderer->bmp.SetPixel(ipos.x, ipos.y, static_cast<PalIdx>(i->color));
        }
      }

      // ---- Minimap (Slice 3e), gated on render_hud && settings->map. Ported from
      //      viewport.cpp:593-613 (terrain miniature + worm dots; the Holdazone
      //      marker :615-634 is deferred — KillEmAll scenarios only). Drawn AFTER
      //      the world block into the RESTORED full-surface clip, INSIDE the
      //      viewport loop so both viewports draw it at the SAME centred position
      //      and the second overwrites the first (spec §7 Q6) — 1:1 with the Rust
      //      frame::draw minimap call (render/src/frame.rs:126-134). ----
      if (scn.render_hud && game.settings->map) {
        renderer->bmp.clip_rect = Rect(0, 0, renderer->bmp.w, renderer->bmp.h);
        int const kCenterX = renderer->bmp.w / 2;  // viewport.cpp:82
        int const kMapX = kCenterX - 26;           // viewport.cpp:594
        int const kMapY = renderer->bmp.h - 38;    // viewport.cpp:595
        int const kMinimapStepX =
            std::max((game.level.width + Level::kHudMinimapW - 1) / Level::kHudMinimapW, 1);
        int const kMinimapStepY =
            std::max((game.level.height + Level::kHudMinimapH - 1) / Level::kHudMinimapH, 1);
        game.level.DrawMiniature(renderer->bmp, kMapX, kMapY, kMinimapStepX, kMinimapStepY);

        for (auto& worm_ptr : game.worms) {  // viewport.cpp:604-613
          Worm const& w = *worm_ptr;
          if (w.visible) {
            int const kX = Ftoi(w.pos.x) / kMinimapStepX + kMapX;
            int const kY = Ftoi(w.pos.y) / kMinimapStepY + kMapY;
            renderer->bmp.SetPixel(kX, kY, w.MinimapColor());
          }
        }
      }
    }

    // Restore the draw-time shadow flip before the next tick's sim Process (spec O2).
    game.settings->shadow = saved_shadow;
    // Restore the draw-time map flip before the next tick's sim Process (Slice 3e).
    game.settings->map = saved_map;

    renderer->bmp.clip_rect = Rect(0, 0, renderer->bmp.w, renderer->bmp.h);
    int const fade = (tick == 0) ? 0 : 33;  // frame 0 black, then identity.
    uint64_t h = kFnvOffset;
    for (int y = 0; y < renderer->bmp.h; ++y) {
      for (int x = 0; x < renderer->bmp.w; ++x) {
        uint32_t const c = renderer->bmp.GetPixel(x, y);
        h = FnvByte(h, FadeChannel((c >> 16) & 0xFF, fade));
        h = FnvByte(h, FadeChannel((c >> 8) & 0xFF, fade));
        h = FnvByte(h, FadeChannel(c & 0xFF, fade));
      }
    }
    frames_acc = (frames_acc ^ h) * kFnvPrime;
    ++frame_count;
    std::fprintf(frames, "%d %016" PRIx64 " %08x\n", tick, h, HashGameState(game));
  };

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
  if (renderer) render_and_hash(0);

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
    if (renderer) render_and_hash(t + 1);
  }

  if (frames) {
    std::fprintf(frames, "total %d %016" PRIx64 "\n", frame_count, frames_acc);
    std::fclose(frames);
  }

  std::fclose(out);
  return 0;
}
