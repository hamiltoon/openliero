// Generates the C++ side of the Rust shell gate G2 and the 4½d MILESTONE (Step 4½, slice 4½d;
// design §6.2-§6.6; rust/oracle-tests/tests/shell_golden.rs). The REAL Gfx::RunOneFrame runs
// headlessly — the REAL StateStack, MainMenuState, GamePlayState, LocalController (weapon
// selection with its 12/3 repeat, the Esc fade, game over), Game::ProcessFrame / Draw,
// UpdateMenuPalettes, DrawBasicMenu and Flip -> Gfx::Draw -> ScaleDraw into a 320x200 ARGB surface
// through a software renderer. Each frame the dumper pushes that frame's script key events
// (SDL_PushEvent) and calls RunOneFrame; it writes a `boot` line after InitFrameStepping, one
// `f` line per frame and an `end` line (formats: docs/superpowers/plans/
// 2026-09-26-liero-rs-step4.5-slice4.5d-plan.md, Task 8).
//
// Interventions, each at a point where no real code runs:
//   1. boot seed: gfx.rand.Seed(boot_seed) before InitFrameStepping (C++ seeds from the clock);
//   2. match seeds: gfx.rand.Seed(<next match seed>) before every frame whose top is the main menu
//      (nothing in the menu draws gfx.rand; only the router's GenerateFromSettings does);
//   3. game seed: after a frame that made a new controller, its Game's rand.Seed(<that seed>). The
//      Game ctor seeds from time(nullptr) and GamePlayState::Enter already ran the
//      WeaponSelection constructor: the reseed is exact only if it drew nothing, which is CHECKED
//      (the rand must still equal a fresh Rand seeded with a time value of that frame);
//   4. no stats screen: that Game's stats_recorder becomes the base StatsRecorder, so game over
//      pops to the menu (gamePlayState.cpp:57-71, :93); 4½g drops this;
//   5. pacing: gfx.last_frame = 0 before each frame; Flip adds 14 per present
//      (gfx.cpp:1176-1189), so presents = last_frame / 14;
//   6. no replays: settings->record_replays = false (localController.cpp:237);
//   7. sounds: gfx.sound_player is a RecordingSoundPlayer (every Game installs it globally);
//   8. the frames run with CWD = data/TC/openliero, so a TC-relative level_file resolves as the
//      Rust port's read_asset does (level.cpp:401-411); a setup whose level file does not open
//      from there is refused. Common, the menus and the setup are loaded before the chdir.
// Usage (from the repo root): oracle_dump_shell <script.txt> <out.txt> [--ppm-dir <dir>]
// Built via OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_shell_golden.sh). Not part of the
// default build.
#include <SDL3/SDL.h>

#include <algorithm>
#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>

#include "common.hpp"
#include "controller/controller.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "gamePlayState.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "keys.hpp"
#include "mainMenuState.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "rand.hpp"
#include "settings.hpp"
#include "state.hpp"
#include "stats_recorder.hpp"
#include "weapsel_drive.hpp"

namespace {

using weapsel_drive::Fail;
using weapsel_drive::RecordingSoundPlayer;

constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;
constexpr int kIdentityFade = 33;
constexpr int kW = 320;
constexpr int kH = 200;

uint64_t FnvByte(uint64_t h, uint8_t b) { return (h ^ b) * kFnvPrime; }

uint8_t FadeChannel(uint8_t v, int amount) {
  return amount >= 32 ? v : static_cast<uint8_t>((v * amount) >> 5);
}

uint64_t HashBitmap(Bitmap const& bmp, int fade) {
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

uint32_t SurfacePixel(SDL_Surface const& s, int x, int y) {
  auto const* row = static_cast<uint8_t const*>(s.pixels) + (static_cast<std::size_t>(y) * s.pitch);
  uint32_t c = 0;
  std::memcpy(&c, row + (static_cast<std::size_t>(x) * 4), sizeof c);
  return c;
}

// The presented frame: what Gfx::Draw left in sdl_draw_surface (ARGB8888, already faded).
uint64_t HashSurface(SDL_Surface const& s) {
  uint64_t h = kFnvOffset;
  for (int y = 0; y < s.h; ++y) {
    for (int x = 0; x < s.w; ++x) {
      uint32_t const kC = SurfacePixel(s, x, y);
      h = FnvByte(h, (kC >> 16) & 0xFFU);
      h = FnvByte(h, (kC >> 8) & 0xFFU);
      h = FnvByte(h, kC & 0xFFU);
    }
  }
  return h;
}

std::string Hex16(uint64_t v) {
  std::array<char, 24> buf{};
  std::snprintf(buf.data(), buf.size(), "%016" PRIx64, v);
  return buf.data();
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

void WriteSurfacePpm(std::string const& path, SDL_Surface const& s) {
  std::string data = "P6\n" + std::to_string(s.w) + " " + std::to_string(s.h) + "\n255\n";
  for (int y = 0; y < s.h; ++y) {
    for (int x = 0; x < s.w; ++x) {
      uint32_t const kC = SurfacePixel(s, x, y);
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

SDL_Scancode ScancodeOf(std::string const& t) {
  static std::map<std::string, SDL_Scancode> const kNames = {
      {"ESC", SDL_SCANCODE_ESCAPE},
      {"RETURN", SDL_SCANCODE_RETURN},
      {"KP_ENTER", SDL_SCANCODE_KP_ENTER},
      {"UP", SDL_SCANCODE_UP},
      {"DOWN", SDL_SCANCODE_DOWN},
      {"LEFT", SDL_SCANCODE_LEFT},
      {"RIGHT", SDL_SCANCODE_RIGHT},
      {"PAGEUP", SDL_SCANCODE_PAGEUP},
      {"PAGEDOWN", SDL_SCANCODE_PAGEDOWN},
      {"LCTRL", SDL_SCANCODE_LCTRL},
      {"RCTRL", SDL_SCANCODE_RCTRL},
      {"LALT", SDL_SCANCODE_LALT},
      {"RALT", SDL_SCANCODE_RALT},
      {"LSHIFT", SDL_SCANCODE_LSHIFT},
      {"RSHIFT", SDL_SCANCODE_RSHIFT},
      {"SPACE", SDL_SCANCODE_SPACE},
      {"TAB", SDL_SCANCODE_TAB},
      {"F1", SDL_SCANCODE_F1},
      {"F2", SDL_SCANCODE_F2},
      {"F3", SDL_SCANCODE_F3},
      {"F4", SDL_SCANCODE_F4},
      {"F5", SDL_SCANCODE_F5},
      {"F6", SDL_SCANCODE_F6},
      {"F7", SDL_SCANCODE_F7},
      {"F8", SDL_SCANCODE_F8},
      {"F9", SDL_SCANCODE_F9},
      {"F10", SDL_SCANCODE_F10},
      {"F11", SDL_SCANCODE_F11},
      {"F12", SDL_SCANCODE_F12},
  };
  auto const kIt = kNames.find(t);
  if (kIt != kNames.end()) {
    return kIt->second;
  }
  if (t.size() == 1 && t[0] >= 'A' && t[0] <= 'Z') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_A + (t[0] - 'A'));
  }
  if (t.size() == 1 && t[0] >= '1' && t[0] <= '9') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_1 + (t[0] - '1'));
  }
  if (t == "0") {
    return SDL_SCANCODE_0;
  }
  Fail("unknown key name " + t);
}

struct KeyEv {
  int frame = 0;
  bool down = false;
  bool repeat = false;
  SDL_Scancode sc = SDL_SCANCODE_UNKNOWN;
};

struct Case {
  std::string setup = "default";
  uint32_t boot_seed = 0;
  bool boot_given = false;
  std::vector<uint32_t> match_seeds;
  int frames = -1;
  std::string expect;
  std::vector<KeyEv> keys;
};

Case ParseCase(std::string const& path) {
  std::istringstream in(Slurp(path));
  Case c;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key) || key[0] == '#') {
      continue;
    }
    if (key == "setup") {
      ls >> c.setup;
    } else if (key == "boot_seed" && (ls >> c.boot_seed)) {
      c.boot_given = true;
    } else if (key == "match_seed") {
      uint32_t s = 0;
      if (!(ls >> s)) {
        Fail("bad match_seed line: " + line);
      }
      c.match_seeds.push_back(s);
    } else if (key == "frames") {
      if (!(ls >> c.frames)) {
        Fail("bad frames line: " + line);
      }
    } else if (key == "expect") {
      if (!(ls >> c.expect) || (c.expect != "quit" && c.expect != "frames")) {
        Fail("bad expect line: " + line);
      }
    } else if (key == "key") {
      KeyEv k;
      std::string kind;
      std::string name;
      if (!(ls >> k.frame >> kind >> name) || k.frame < 0 ||
          (kind != "down" && kind != "up" && kind != "repeat")) {
        Fail("bad key line: " + line);
      }
      k.down = kind != "up";
      k.repeat = kind == "repeat";
      k.sc = ScancodeOf(name);
      c.keys.push_back(k);
    } else {
      Fail("bad script line: " + line);
    }
  }
  if (!c.boot_given || c.frames <= 0 || c.expect.empty()) {
    Fail("a shell script needs boot_seed, frames and expect");
  }
  std::stable_sort(c.keys.begin(), c.keys.end(),
                   [](KeyEv const& a, KeyEv const& b) { return a.frame < b.frame; });
  return c;
}

void Push(KeyEv const& k) {
  SDL_Event ev{};
  ev.type = k.down ? SDL_EVENT_KEY_DOWN : SDL_EVENT_KEY_UP;
  ev.key.scancode = k.sc;
  ev.key.key = SDL_GetKeyFromScancode(k.sc, SDL_KMOD_NONE, false);
  ev.key.mod = SDL_KMOD_NONE;
  ev.key.down = k.down;
  ev.key.repeat = k.repeat;
  if (!SDL_PushEvent(&ev)) {
    Fail(std::string("SDL_PushEvent: ") + SDL_GetError());
  }
}

char TopOf(AppState* s) {
  if (s == nullptr) {
    return '-';
  }
  if (dynamic_cast<MainMenuState*>(s) != nullptr) {
    return 'M';
  }
  if (dynamic_cast<GamePlayState*>(s) != nullptr) {
    return 'G';
  }
  Fail("a state 4½d does not model is on the stack");
}

// The fields every line shares: bmp16 fade menu_cycles top sel.
std::string Tail() {
  return Hex16(HashBitmap(gfx.play_renderer.bmp, kIdentityFade)) + " " +
         std::to_string(gfx.play_renderer.fade_value) + " " + std::to_string(gfx.menu_cycles) +
         " " + TopOf(gfx.state_stack.Top()) + " " + std::to_string(gfx.main_menu.Selection());
}

std::string Presented(std::string const& ppm_dir, std::string const& name) {
  uint64_t const kPresents = gfx.last_frame / 14;
  if (kPresents > 1) {
    Fail("more than one present in a frame");
  }
  if (kPresents == 0) {
    return "0 -";
  }
  if (!ppm_dir.empty()) {
    WriteSurfacePpm(ppm_dir + "/" + name + ".ppm", *gfx.sdl_draw_surface);
  }
  return "1 " + Hex16(HashSurface(*gfx.sdl_draw_surface));
}

}  // namespace

int main(int argc, char** argv) {
  std::vector<std::string> const kArgs(argv + 1, argv + argc);
  std::vector<std::string> positional;
  std::string ppm_dir;
  for (std::size_t i = 0; i < kArgs.size(); ++i) {
    if (kArgs[i] == "--ppm-dir" && i + 1 < kArgs.size()) {
      ppm_dir = std::filesystem::absolute(kArgs[++i]).string();
    } else if (kArgs[i].starts_with("--")) {
      Fail("unknown or incomplete option " + kArgs[i]);
    } else {
      positional.push_back(kArgs[i]);
    }
  }
  if (positional.size() != 2) {
    Fail("usage: oracle_dump_shell <script.txt> <out.txt> [--ppm-dir <dir>]");
  }
  std::string const kScript = positional[0];
  std::string const kOut = std::filesystem::absolute(positional[1]).string();
  Case const kCase = ParseCase(kScript);

  // GameEntry's setup (gameEntry.cpp:21-76), headless.
  if (!SDL_Init(SDL_INIT_EVENTS)) {
    Fail(std::string("SDL_Init: ") + SDL_GetError());
  }
  InitKeys();
  PrecomputeTables();
  gfx.LoadMenus();
  auto settings = std::make_shared<Settings>();
  if (kCase.setup != "default") {
    try {
      settings->FromToml(Slurp(DirOf(kScript) + "/" + kCase.setup));
    } catch (std::exception const& e) {
      Fail("setup " + kCase.setup + ": " + e.what());
    }
  }
  settings->record_replays = false;  // intervention 6
  gfx.settings = settings;
  gfx.settings_node = FsNode("data") / "Setups" / "liero.cfg";
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");
  gfx.common = common;
  gfx.play_renderer.Init(kW, kH);
  gfx.play_renderer.LoadPalette(*common);
  gfx.single_screen_renderer.Init(640, 400);
  ColorMode const kMode = settings->modern_colors ? ColorMode::kModern : ColorMode::kClassic;
  gfx.play_renderer.mode = kMode;
  gfx.single_screen_renderer.mode = kMode;

  // Headless presentation (design §6.2 step 4): a 320x200 draw surface makes FitScreen pick
  // magnification 1 at offset 0 (blit.cpp:860-876), so the real Flip -> Gfx::Draw -> ScaleDraw
  // writes exactly the faded play_renderer frame into it.
  gfx.sdl_draw_surface = SDL_CreateSurface(kW, kH, SDL_PIXELFORMAT_ARGB8888);
  SDL_Surface* target = SDL_CreateSurface(kW, kH, SDL_PIXELFORMAT_ARGB8888);
  gfx.sdl_renderer = target != nullptr ? SDL_CreateSoftwareRenderer(target) : nullptr;
  gfx.sdl_texture = gfx.sdl_renderer != nullptr
                        ? SDL_CreateTexture(gfx.sdl_renderer, SDL_PIXELFORMAT_ARGB8888,
                                            SDL_TEXTUREACCESS_STREAMING, kW, kH)
                        : nullptr;
  if (gfx.sdl_draw_surface == nullptr || gfx.sdl_texture == nullptr) {
    Fail(std::string("headless presentation: ") + SDL_GetError() +
         " (plan T8: try the dummy-video fallback)");
  }

  auto rec = std::make_shared<RecordingSoundPlayer>();  // intervention 7
  gfx.sound_player = rec;
  g_sound_player = rec.get();

  std::filesystem::current_path("data/TC/openliero");  // intervention 8
  if (!settings->random_level) {
    std::string path = settings->level_file;
    if (!path.contains('.')) {
      path += ".LEV";
    }
    if (!std::ifstream(path).good()) {
      Fail("the setup's level file " + path + " does not open from data/TC/openliero");
    }
  }

  std::string out = "# oracle_dump_shell " + kScript +
                    " — the REAL C++ Gfx frame loop, headless (Step 4½d design §6.2)\n"
                    "# boot <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel>\n"
                    "# f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> "
                    "<top> <sel> <sounds>\n";
  gfx.rand.Seed(kCase.boot_seed);  // intervention 1
  gfx.last_frame = 0;              // intervention 5
  gfx.InitFrameStepping();
  out += "boot " + Presented(ppm_dir, "boot") + " " + Tail() + "\n";

  std::size_t next_seed = 0;
  std::size_t next_key = 0;
  int end = kCase.frames;
  bool quit = false;
  for (int frame = 0; frame < kCase.frames; ++frame) {
    char const kTop = TopOf(gfx.state_stack.Top());
    if (kTop == 'M' && next_seed < kCase.match_seeds.size()) {
      gfx.rand.Seed(kCase.match_seeds[next_seed]);  // intervention 2
    }
    char upd = kTop;
    if (kTop == 'G') {
      upd = gfx.controller->InWeaponSelection() ? 'W' : 'G';
    }
    while (next_key < kCase.keys.size() && kCase.keys[next_key].frame == frame) {
      Push(kCase.keys[next_key++]);
    }
    Controller const* const kBefore = gfx.controller.get();
    std::time_t const kT0 = std::time(nullptr);
    rec->played.clear();
    gfx.last_frame = 0;  // intervention 5
    bool const kGo = gfx.RunOneFrame();
    std::time_t const kT1 = std::time(nullptr);
    if (gfx.controller.get() != kBefore) {
      // A NEW GAME made a controller: interventions 3 and 4.
      std::string const kAt = "frame " + std::to_string(frame) + ": ";
      if (next_seed >= kCase.match_seeds.size()) {
        Fail(kAt + "a NEW GAME with no match_seed left");
      }
      if (settings->game_mode == Settings::kGmHoldazone) {
        Fail(kAt +
             "a Holdazone match (unported in Rust): use Holdazone setups for boot cases only");
      }
      Game& game = *gfx.controller->CurrentGame();
      bool untouched = false;
      for (std::time_t t = kT0 - 1; t <= kT1 + 1; ++t) {
        Rand probe;
        probe.Seed(static_cast<uint32_t>(t));
        untouched = untouched || (probe.engine == game.rand.engine && probe.last == game.rand.last);
      }
      if (!untouched) {
        Fail(kAt + "the WeaponSelection constructor drew the RNG (intervention 3's precondition)");
      }
      game.rand.Seed(kCase.match_seeds[next_seed++]);
      game.stats_recorder = std::make_shared<StatsRecorder>();
    }
    std::string sounds;
    for (int const kId : rec->played) {
      sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
    }
    std::array<char, 16> name{};
    std::snprintf(name.data(), name.size(), "f_%04d", frame);
    out += "f " + std::to_string(frame) + " " + upd + " " + Presented(ppm_dir, name.data()) + " " +
           Tail() + " " + (sounds.empty() ? "-" : sounds) + "\n";
    if (!kGo) {
      end = frame;
      quit = true;
      break;
    }
  }
  if (next_key != kCase.keys.size()) {
    Fail("key events scheduled after the run ended");
  }
  if (next_seed != kCase.match_seeds.size()) {
    Fail("unused match seeds");
  }
  if ((kCase.expect == "quit") != quit) {
    Fail("expected to end by " + kCase.expect);
  }
  out += "end " + std::to_string(end) + " " + (quit ? "quit" : "frames") + "\n";

  std::ofstream f(kOut, std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + kOut);
  }
  std::printf("oracle_dump_shell: %d frames, %zu NEW GAMEs, end %s\n", quit ? end + 1 : end,
              next_seed, quit ? "quit" : "frames");
  gfx.controller.reset();
  g_sound_player = nullptr;
  return 0;
}
