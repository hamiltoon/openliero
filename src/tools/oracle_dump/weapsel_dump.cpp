// Generates the C++ side of the Rust weapon-selection gate (Step 4½, slice 4½c;
// rust/oracle-tests/tests/weapsel_golden.rs). For one scenario it runs the REAL C++
// WeaponSelection (weapsel.cpp) — the constructor, one ProcessFrame per `weapsel` frame,
// Finalize — through the shared driver (weapsel_drive.hpp) and writes one line per step:
//   init <enabled> <p0> <p1> <draws> <last> <next>
//   f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> <draws> <last> <next>
//     <done>
//   final <l0> <l1> <ctl0> <ctl1> <last> <next>
// pN = the five 1-based picks (WormSettings::weapons), ',' joined, then ':cursor:ready'; inN = the
// masked 7-bit word; ctlN = the worm's control word after the step (%02x); heldN = the seven
// key-repeat counters; sounds = sample ids in Play order or '-'; draws = RNG steps in the step;
// last/next = rand.last and the next raw value (%08x); lN = five 'weapon_id:ammo' pairs, ','
// joined, then ':current_weapon' (after Finalize).
//
// Usage (run from the repo root):
//   oracle_dump_weapsel <scenario.txt> <out.txt> [--frames <frames.txt>] [--menu-cycles <n>]
//                       [--ppm-dir <dir>]
// The scenario is a `settings` scenario (seed, level, ticks, settings, weapsel, input, comments;
// `ticks`/`input` are match ticks, oracle_dump_sim_physics's business). The setup is read with
// the REAL Settings::FromToml and the worms start exactly as sim_physics_dump's settings path
// starts them (health = ws.health, stats_x 0/218, no InitWeapons).
//
// Render mode (plan Addendum A1; opt-in with --frames and/or --ppm-dir; the golden text is
// byte-identical with or without it): the REAL WeaponSelection::Draw(gfx.play_renderer,
// kStateWeaponSelection, false) runs headlessly after every ProcessFrame that leaves the phase
// running, with exactly the gfx state the real game gives it:
//   * gfx.play_renderer: Init(320, 200) + LoadPalette(common) + the settings' colour mode, as
//     Gfx::Init / gameEntry.cpp:69-70 set it up;
//   * gfx.settings = the game's settings (LocalController shares gfx.settings; DrawNormalViewports
//     reads gfx.settings->level_file for the level label);
//   * game.Focus(gfx.play_renderer) after the constructor, as LocalController::Focus does right
//     after ChangeState(kStateWeaponSelection) (localController.cpp:106-115): the level palette +
//     Palette::SetWormColour for both worms;
//   * gfx.frozen_screen empty at the phase start (cached_background is per WeaponSelection);
//   * gfx.menu_cycles = <n> (default 0) at the first draw, then ++ after every draw, as
//     Gfx::RunOneFrame does for GamePlayState (WantsMenuFlip() == false: ++menu_cycles after
//     state_stack.Draw(), gfx.cpp:1632-1647);
//   * play_renderer.Clear() before the draw (GamePlayState::Draw, gamePlayState.cpp:99-101).
// The frame on which ProcessFrame returns true is NOT drawn: LocalController::Process has already
// changed state to kStateGame, and Draw then draws the match (localController.cpp:150-151,
// :158-165). --frames writes one line per drawn frame:
//   <frame> <frame_hash16> <menu_cycles> <fade>
// frame_hash16 is the render goldens' hash_frame (framehash_main.cpp, render/src/hash.rs) of the
// 320x200 play_renderer bitmap at fade 33 (identity: the pixels as resolved through the weapsel
// palette); <menu_cycles> is the value the draw read; <fade> is the composition fade the real game
// shows the frame with (LocalController::fade_value = min(frame + 1, 33): Focus zeroes it and
// every Process adds 1, localController.cpp:115, :181-183) — hash_frame(bmp, fade) is a pure
// function of the identity pixels, so the identity hash pins the faded frame too. --ppm-dir
// writes each drawn frame as <dir>/weapsel_<frame, 4 digits>.ppm (binary P6, identity colours).
//
// Self-check (exits 1 without writing): the case is replayed through a REAL LocalController
// (OnKey + Process, localController.cpp) from a fresh read of the same setup and seed. It must
// agree with the driver on picks, cursors, ready flags, control words, repeat counters and the RNG
// after every frame, and on the loaded weapons after Finalize. A changed bit is sent as the
// GamepadControlToExKey(worm, control) key (keys.hpp:26-28), which Game::FindControlForKey maps
// straight to (worm, control) whatever the setup's key bindings are (game.cpp:75-85). Built via
// OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_weapsel_golden.sh). Not part of the default
// build.
#include <algorithm>
#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <fstream>
#include <iterator>
#include <memory>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "common.hpp"
#include "controller/localController.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "gfx/renderer.hpp"
#include "io/stream.hpp"
#include "keys.hpp"
#include "level.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "stats_recorder.hpp"
#include "weapsel.hpp"
#include "weapsel_drive.hpp"
#include "worm.hpp"

namespace {

using weapsel_drive::Fail;

// FNV-1a frame hash + the composition fade, as framehash_main.cpp:26-34 and
// sim_physics_dump.cpp's render path (render/src/hash.rs on the Rust side).
constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;
constexpr int kIdentityFade = 33;
constexpr int kFullFade = 33;  // LocalController::Process caps fade_value here.

uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }

uint8_t FadeChannel(uint8_t v, int amount) {
  return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
}

uint64_t HashFrame(Bitmap const& bmp, int fade) {
  uint64_t h = kFnvOffset;
  for (int y = 0; y < bmp.h; ++y) {
    for (int x = 0; x < bmp.w; ++x) {
      uint32_t const kC = bmp.GetPixel(x, y);
      h = FnvByte(h, FadeChannel((kC >> 16) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel((kC >> 8) & 0xFFU, fade));
      h = FnvByte(h, FadeChannel(kC & 0xFFU, fade));
    }
  }
  return h;
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

// Writes `bmp` as a binary PPM (P6), identity colours (for eyeballing only).
void WritePpm(std::string const& path, Bitmap const& bmp) {
  std::string data = "P6\n" + std::to_string(bmp.w) + " " + std::to_string(bmp.h) + "\n255\n";
  data.reserve(data.size() + (static_cast<std::size_t>(bmp.w) * bmp.h * 3));
  for (int y = 0; y < bmp.h; ++y) {
    for (int x = 0; x < bmp.w; ++x) {
      uint32_t const kC = bmp.GetPixel(x, y);
      data += static_cast<char>((kC >> 16) & 0xFFU);
      data += static_cast<char>((kC >> 8) & 0xFFU);
      data += static_cast<char>(kC & 0xFFU);
    }
  }
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  f << data;
  if (!f) {
    Fail("cannot write " + path);
  }
}

// The directory part of `path` ("." when there is none): the setup resolves relative to it.
std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

struct Scenario {
  uint32_t seed = 0;
  bool seed_given = false;
  std::string level;
  std::string settings_file;
  weapsel_drive::Script weapsel;
};

Scenario ParseScenario(std::string const& path) {
  std::istringstream in(Slurp(path));
  Scenario s;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key) || key[0] == '#') {
      continue;
    }
    if (key == "seed") {
      ls >> s.seed;
      s.seed_given = true;
    } else if (key == "level") {
      ls >> s.level;
    } else if (key == "settings") {
      ls >> s.settings_file;
    } else if (key == "weapsel") {
      // Exactly 3 numbers, like the Rust parser (a `#` token starts a comment).
      int frame = -1;
      std::array<uint32_t, 2> words{0, 0};
      std::string extra;
      if (!(ls >> frame >> words[0] >> words[1]) || frame < 0 || (ls >> extra && extra[0] != '#')) {
        Fail("bad weapsel line: " + line);
      }
      if (!s.weapsel.emplace(frame, words).second) {
        Fail("duplicate weapsel frame: " + line);
      }
    } else if (key != "ticks" && key != "input") {
      Fail("unsupported directive for oracle_dump_weapsel: " + key);
    }
  }
  if (!s.weapsel.empty() && s.settings_file.empty()) {
    Fail("weapsel is oracle-only: it needs a settings directive");
  }
  if (!s.seed_given || s.level.empty() || s.settings_file.empty() || s.weapsel.empty()) {
    Fail("a weapsel scenario needs seed, level, settings and at least one weapsel line");
  }
  return s;
}

std::shared_ptr<Settings> LoadSettings(std::string const& path) {
  auto settings = std::make_shared<Settings>();
  try {
    settings->FromToml(Slurp(path));
  } catch (std::exception const& e) {
    Fail("settings " + path + ": " + e.what());
  }
  if (settings->game_mode == Settings::kGmHoldazone) {
    // As sim_physics_dump: unported in Rust (and StartGame's SpawnZone would read the level).
    Fail("settings " + path + ": Holdazone is refused");
  }
  return settings;
}

std::string Hex(uint32_t v, int width) {
  std::array<char, 16> buf{};
  std::snprintf(buf.data(), buf.size(), "%0*x", width, v);
  return buf.data();
}

template <typename T, std::size_t N>
std::string Join(std::array<T, N> const& v) {
  std::string out;
  for (std::size_t i = 0; i < N; ++i) {
    if (i != 0) {
      out += ',';
    }
    out += std::to_string(v[i]);
  }
  return out;
}

std::array<uint32_t, 5> Picks(Game const& game, std::size_t i) {
  std::array<uint32_t, 5> p{};
  for (std::size_t j = 0; j < p.size(); ++j) {
    p[j] = game.worms[i]->settings->weapons[j];
  }
  return p;
}

std::string PlayerField(Game const& game, WeaponSelection const& ws, std::size_t i) {
  return Join(Picks(game, i)) + ":" + std::to_string(ws.menus[i].Selection()) + ":" +
         (ws.is_ready[i] ? "1" : "0");
}

std::array<std::pair<int, int>, 5> Loadout(Game const& game, std::size_t i) {
  std::array<std::pair<int, int>, 5> l{};
  for (std::size_t j = 0; j < l.size(); ++j) {
    WormWeapon const& ww = game.worms[i]->weapons[j];
    l[j] = {ww.type->id, ww.ammo};
  }
  return l;
}

// Everything the self-check compares after one step.
struct Snap {
  std::array<std::array<uint32_t, 5>, 2> picks{};
  std::array<int, 2> cursor{};
  std::array<bool, 2> ready{};
  std::array<uint32_t, 2> ctl{};
  std::array<std::array<uint16_t, weapsel_drive::kRepeatBits>, 2> held{};
  std::array<std::array<std::pair<int, int>, 5>, 2> loadout{};
  Rand rand;
};

// The design §6.2 self-check: the same case through a REAL LocalController. Exits 1 on the first
// disagreement.
void SelfCheck(std::shared_ptr<Common> const& common, std::string const& cfg_path,
               Scenario const& scn, std::vector<Snap> const& want, Snap const& want_final) {
  gfx.sound_player = std::make_shared<NullSoundPlayer>();
  auto settings = LoadSettings(cfg_path);
  // Keep ChangeState(kStateGame) off the filesystem (localController.cpp:237). Not sim-reaching.
  settings->record_replays = false;
  LocalController lc(common, settings);
  lc.game.stats_recorder = std::make_shared<StatsRecorder>();  // StartGame's Reset: a no-op
  lc.game.rand.Seed(scn.seed);
  lc.Focus();  // kStateInitial -> ChangeState(kStateWeaponSelection): the REAL constructor
  int const kEnd = scn.weapsel.rbegin()->first;
  std::array<uint32_t, 2> prev{0, 0};
  for (int f = 0; f <= kEnd; ++f) {
    std::array<uint32_t, 2> words{0, 0};
    auto const kIt = scn.weapsel.find(f);
    if (kIt != scn.weapsel.end()) {
      words = {kIt->second[0] & 0x7FU, kIt->second[1] & 0x7FU};
    }
    for (std::size_t wi = 0; wi < 2; ++wi) {
      uint32_t const kChanged = words[wi] ^ prev[wi];
      for (std::size_t bit = 0; bit < weapsel_drive::kRepeatBits; ++bit) {
        if (((kChanged >> bit) & 1U) != 0) {
          lc.OnKey(
              static_cast<int>(GamepadControlToExKey(static_cast<int>(wi), static_cast<int>(bit))),
              ((words[wi] >> bit) & 1U) != 0);
        }
      }
      prev[wi] = words[wi];
    }
    lc.Process();
    bool const kLast = f == kEnd;
    std::string const kAt = "self-check frame " + std::to_string(f) + ": ";
    if (kLast != (lc.state == kStateGame)) {
      Fail(kAt + "LocalController left weapon selection on a different frame");
    }
    Snap const& w = kLast ? want_final : want[f];
    for (std::size_t i = 0; i < 2; ++i) {
      if (Picks(lc.game, i) != want[f].picks[i]) {
        Fail(kAt + "picks differ for player " + std::to_string(i));
      }
      if (lc.game.worms[i]->control_states.Pack() != w.ctl[i]) {
        Fail(kAt + "control word differs for worm " + std::to_string(i));
      }
      if (lc.worm_held_frames[i] != want[f].held[i]) {
        Fail(kAt + "repeat counters differ for worm " + std::to_string(i));
      }
      if (!kLast && (lc.ws->menus[i].Selection() != want[f].cursor[i] ||
                     static_cast<bool>(lc.ws->is_ready[i]) != want[f].ready[i])) {
        Fail(kAt + "cursor or ready differs for player " + std::to_string(i));
      }
      if (kLast && Loadout(lc.game, i) != want_final.loadout[i]) {
        Fail(kAt + "loaded weapons differ for worm " + std::to_string(i));
      }
    }
    if (lc.game.rand != w.rand) {
      Fail(kAt + "the RNG differs");
    }
  }
}

struct Options {
  std::string scenario;
  std::string out;
  std::string frames;   // --frames: the per-frame hash sidecar (render mode)
  std::string ppm_dir;  // --ppm-dir: one PPM per drawn frame (render mode)
  unsigned menu_cycles = 0;
  bool Render() const { return !frames.empty() || !ppm_dir.empty(); }
};

Options ParseArgs(int argc, char** argv) {
  std::vector<std::string> const kArgs(argv + 1, argv + argc);
  Options o;
  std::vector<std::string> positional;
  for (std::size_t i = 0; i < kArgs.size(); ++i) {
    std::string const& a = kArgs[i];
    bool const kHasValue = i + 1 < kArgs.size();
    if (a == "--frames" && kHasValue) {
      o.frames = kArgs[++i];
    } else if (a == "--ppm-dir" && kHasValue) {
      o.ppm_dir = kArgs[++i];
    } else if (a == "--menu-cycles" && kHasValue) {
      o.menu_cycles = static_cast<unsigned>(std::strtoul(kArgs[++i].c_str(), nullptr, 10));
    } else if (a.starts_with("--")) {
      Fail("unknown or incomplete option " + a);
    } else {
      positional.push_back(a);
    }
  }
  if (positional.size() != 2) {
    Fail(
        "usage: oracle_dump_weapsel <scenario.txt> <out.txt> [--frames <frames.txt>] "
        "[--menu-cycles <n>] [--ppm-dir <dir>]");
  }
  o.scenario = positional[0];
  o.out = positional[1];
  return o;
}

}  // namespace

int main(int argc, char** argv) {
  Options const kOpt = ParseArgs(argc, argv);
  Scenario const kScn = ParseScenario(kOpt.scenario);
  std::string const kCfgPath = DirOf(kOpt.scenario) + "/" + kScn.settings_file;

  PrecomputeTables();
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");

  // The sim_physics_dump settings path's setup (sim_physics_dump.cpp: FromToml, the no-op
  // StatsRecorder, the StartGame pool, Seed, Level::load, then the two LocalController worms).
  auto settings = LoadSettings(kCfgPath);
  auto sound = std::make_shared<weapsel_drive::RecordingSoundPlayer>();
  Game game(common, settings, sound);
  game.stats_recorder = std::make_shared<StatsRecorder>();
  game.bobjects.Resize(settings->blood_particle_max);
  game.rand.Seed(kScn.seed);
  {
    std::string const kLevelPath = "data/TC/openliero/" + kScn.level;
    std::string const kBytes = Slurp(kLevelPath);
    std::vector<uint8_t> const kBuf(kBytes.begin(), kBytes.end());
    io::MemReader r(kBuf);
    if (!game.level.load(*common, *settings, r)) {
      Fail("Level::load failed for " + kLevelPath);
    }
  }
  for (int idx = 0; idx < 2; ++idx) {
    auto w = std::make_shared<Worm>();
    w->settings = settings->worm_settings[idx];
    w->health = w->settings->health;
    w->index = idx;
    w->stats_x = idx == 0 ? 0 : 218;
    game.AddWorm(w);
  }

  // Render mode: the gfx state WeaponSelection::Draw reads (see the header comment).
  std::FILE* frames = nullptr;
  if (kOpt.Render()) {
    gfx.settings = settings;
    gfx.play_renderer.Init(320, 200);
    gfx.play_renderer.LoadPalette(*common);
    ColorMode const kMode = settings->modern_colors ? ColorMode::kModern : ColorMode::kClassic;
    gfx.play_renderer.mode = kMode;
    gfx.single_screen_renderer.mode = kMode;
    // gfx.frozen_screen: never allocated in this process (Gfx::Gfx leaves it empty), and the
    // first Draw fills it (cached_background starts false).
    gfx.menu_cycles = kOpt.menu_cycles;
    if (!kOpt.frames.empty()) {
      // NOLINTNEXTLINE(android-cloexec-fopen) — single-threaded CLI tool; no exec, no fd leak.
      frames = std::fopen(kOpt.frames.c_str(), "w");
      if (frames == nullptr) {
        Fail("cannot open " + kOpt.frames);
      }
    }
  }

  std::string out = "# oracle_dump_weapsel " + kOpt.scenario +
                    " — the REAL C++ weapon-selection phase (Step 4½c design §6.3)\n"
                    "# init <enabled> <p0> <p1> <draws> <last> <next>\n"
                    "# f <frame> <in0> <in1> <p0> <p1> <ctl0> <ctl1> <held0> <held1> <sounds> "
                    "<draws> <last> <next> <done>\n"
                    "# final <l0> <l1> <ctl0> <ctl1> <last> <next>\n";
  std::vector<Snap> snaps;
  Rand before = game.rand;
  auto const kSnap = [&](weapsel_drive::Driver const& d) {
    Snap s;
    for (std::size_t i = 0; i < 2; ++i) {
      s.picks[i] = Picks(game, i);
      s.cursor[i] = d.Ws().menus[i].Selection();
      s.ready[i] = d.Ws().is_ready[i];
      s.ctl[i] = game.worms[i]->control_states.Pack();
      s.held[i] = d.Held(i);
    }
    s.rand = game.rand;
    return s;
  };
  int drawn = 0;
  weapsel_drive::Run(
      game, kScn.weapsel,
      [&](weapsel_drive::Driver const& d) {
        uint64_t const kDraws = weapsel_drive::CountDraws(before, game.rand);
        before = game.rand;
        out += "init " + std::to_string(d.Ws().enabled_weaps) + " " + PlayerField(game, d.Ws(), 0) +
               " " + PlayerField(game, d.Ws(), 1) + " " + std::to_string(kDraws) + " " +
               Hex(game.rand.last, 8) + " " + Hex(weapsel_drive::PeekNext(game.rand), 8) + "\n";
        sound->played.clear();
        if (kOpt.Render()) {
          // LocalController::Focus, right after ChangeState(kStateWeaponSelection).
          game.Focus(gfx.play_renderer);
        }
      },
      [&](weapsel_drive::Driver& d, int frame, std::array<uint32_t, 2> const& words, bool done) {
        uint64_t const kDraws = weapsel_drive::CountDraws(before, game.rand);
        before = game.rand;
        std::string sounds;
        for (int const kId : sound->played) {
          sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
        }
        sound->played.clear();
        out += "f " + std::to_string(frame) + " " + std::to_string(words[0]) + " " +
               std::to_string(words[1]) + " " + PlayerField(game, d.Ws(), 0) + " " +
               PlayerField(game, d.Ws(), 1) + " " + Hex(game.worms[0]->control_states.Pack(), 2) +
               " " + Hex(game.worms[1]->control_states.Pack(), 2) + " " + Join(d.Held(0)) + " " +
               Join(d.Held(1)) + " " + (sounds.empty() ? "-" : sounds) + " " +
               std::to_string(kDraws) + " " + Hex(game.rand.last, 8) + " " +
               Hex(weapsel_drive::PeekNext(game.rand), 8) + " " + (done ? "1" : "0") + "\n";
        snaps.push_back(kSnap(d));
        if (kOpt.Render() && !done) {
          unsigned const kCycles = gfx.menu_cycles;
          gfx.play_renderer.Clear();
          d.MutableWs().Draw(gfx.play_renderer, kStateWeaponSelection,
                             /*use_spectator_viewports=*/false);
          ++gfx.menu_cycles;
          Bitmap const& bmp = gfx.play_renderer.bmp;
          int const kFade = std::min(frame + 1, kFullFade);
          if (frames != nullptr) {
            std::fprintf(frames, "%d %016" PRIx64 " %u %d\n", frame, HashFrame(bmp, kIdentityFade),
                         kCycles, kFade);
          }
          if (!kOpt.ppm_dir.empty()) {
            std::array<char, 32> name{};
            std::snprintf(name.data(), name.size(), "/weapsel_%04d.ppm", frame);
            WritePpm(kOpt.ppm_dir + name.data(), bmp);
          }
          ++drawn;
        }
      });
  // Finalize ran inside Run (InitWeapons + ReleaseControls); it must draw nothing (design §3.5).
  if (weapsel_drive::CountDraws(before, game.rand) != 0) {
    Fail("Finalize drew the RNG");
  }
  Snap fin;
  std::array<std::string, 2> loadouts;
  for (std::size_t i = 0; i < 2; ++i) {
    fin.ctl[i] = game.worms[i]->control_states.Pack();
    fin.loadout[i] = Loadout(game, i);
    for (auto const& slot : fin.loadout[i]) {
      loadouts[i] += (loadouts[i].empty() ? "" : ",") + std::to_string(slot.first) + ":" +
                     std::to_string(slot.second);
    }
    loadouts[i] += ":" + std::to_string(game.worms[i]->current_weapon);
  }
  fin.rand = game.rand;
  out += "final " + loadouts[0] + " " + loadouts[1] + " " + Hex(fin.ctl[0], 2) + " " +
         Hex(fin.ctl[1], 2) + " " + Hex(game.rand.last, 8) + " " +
         Hex(weapsel_drive::PeekNext(game.rand), 8) + "\n";
  if (frames != nullptr) {
    std::fclose(frames);
  }

  SelfCheck(common, kCfgPath, kScn, snaps, fin);

  std::ofstream f(kOpt.out, std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + kOpt.out);
  }
  std::printf("oracle_dump_weapsel: %zu frames, LocalController self-check agreed", snaps.size());
  if (kOpt.Render()) {
    std::printf("; %d frames drawn", drawn);
  }
  std::printf("\n");
  return 0;
}
