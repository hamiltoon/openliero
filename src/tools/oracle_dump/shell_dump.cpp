// Generates the C++ side of the Rust shell gates G2 (Step 4½, slice 4½d; design §6.2-§6.6) and
// G2e-1 (slice 4½e-1; rust/oracle-tests/tests/shell_golden.rs). The REAL Gfx::RunOneFrame runs
// headlessly — the REAL StateStack, MainMenuState (and its settings focus), WeaponMenuState,
// InputStringState, InfoBoxState, GamePlayState, LocalController (weapon selection with its 12/3
// repeat, the Esc fade, game over), Game::ProcessFrame / Draw, UpdateMenuPalettes, DrawBasicMenu
// and Flip -> Gfx::Draw -> ScaleDraw into a 320x200 ARGB surface through a software renderer.
// Each frame the dumper pushes that frame's script events (SDL_PushEvent: key events and text
// events, in file order within the frame) and calls RunOneFrame; it writes a `boot` line after
// InitFrameStepping, one `f` line per frame and an `end` line (formats: docs/superpowers/plans/
// 2026-09-26-liero-rs-step4.5-slice4.5d-plan.md, Task 8; the tops O/I/B and the opt-in lines:
// docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5e1-plan.md, §Formats pinned).
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
//   6. no replays: settings->record_replays = false (localController.cpp:237); it also reaches the
//      d lines' cfg16 and the exit save, which the Rust harness mirrors;
//   7. sounds: gfx.sound_player is a RecordingSoundPlayer (every Game installs it globally);
//   8. the frames run with CWD = data/TC/openliero, so a TC-relative level_file resolves as the
//      Rust port's read_asset does (level.cpp:401-411); a setup whose level file does not open
//      from there is refused. Common, the menus and the setup are loaded before the chdir. An `fs`
//      case instead runs with CWD = its fixture, with the same refusal from there;
//   9. the exit save: when an `fs` case ends by quit, gfx.settings->save(user config node /
//      "Setups" / "liero.cfg", gfx.rand) — gameEntry.cpp:78 verbatim, which the dumper never
//      reaches (the Rust side is Shell::save_on_exit).
//
// Opt-in script directives (4½e-1; a script without them writes the 4½d output byte-identically):
//   text <frame> <hex>  one SDL_EVENT_TEXT_INPUT whose string is the (lowercase, 2..8 digit) hex
//                       bytes; the strings live for the whole run. Key names gain BACKSPACE.
//   detail              a `d <frame> <cur> <ssel> <cfg16> <state8|->` line after every f line: cur
//                       is M/S from gfx.cur_menu, ssel settings_menu.Selection(), cfg16 the
//                       FNV-1a-64 of gfx.settings->ToToml(), state8 the %08x HashGameState when
//                       the top after the frame is G outside weapon selection, else '-'.
//   fs <manifest>       (needs `setup default`) the two-layer file-system fixture: the manifest
//                       (golden-dir-relative; `dir <user|sys> <rel>` / `file <user|sys> <rel>
//                       <repo-relative source>`) is copied into <temp>/oracle_shell_fs_<case>/
//                       {user,sys}; after the TC is loaded from the repo the dumper chdirs there,
//                       sets OPENLIERO_TEST_USER_DIR=user and OPENLIERO_DATADIR=sys (environment
//                       and CWD only — no code intervention) and boots through the REAL
//                       paths::Resolve and Gfx::LoadSettings / SaveSettings as gameEntry.cpp:30-58
//                       does. After the end line (and intervention 9) it writes `file <rel>
//                       <fnv16>` for every regular file under user/, rel sorted bytewise, then
//                       removes the fixture.
// The search-gap check (plan D4): C++'s WeaponMenu search clears its prefix after 1500 ms of
// SDL_GetTicks() (menu.cpp:21-24) and the Rust harness passes now_ms = 0, so a case fails when two
// frames of one WeaponMenuState visit (InfoBox interludes included) carry printable key-downs
// >= 1000 ms apart. InputStringState::Enter's SDL_StartTextInput(nullptr window) fails harmlessly
// headless; the pushed text events still arrive.
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
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <deque>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "common.hpp"
#include "controller/controller.hpp"
#include "filesystem.hpp"
#include "game.hpp"
#include "gamePlayState.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "inputState.hpp"
#include "keys.hpp"
#include "mainMenuState.hpp"
#include "math.hpp"
#include "mixer/player.hpp"
#include "rand.hpp"
#include "settings.hpp"
#include "state.hpp"
#include "stateHash.hpp"
#include "stats_recorder.hpp"
#include "weaponMenuState.hpp"
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

uint64_t FnvBytes(std::string_view bytes) {
  uint64_t h = kFnvOffset;
  for (char const kB : bytes) {
    h = FnvByte(h, static_cast<uint8_t>(kB));
  }
  return h;
}

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

std::string Hex8(uint32_t v) {
  std::array<char, 16> buf{};
  std::snprintf(buf.data(), buf.size(), "%08" PRIx32, v);
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
  if (t == "BACKSPACE") {  // 4½e-1: DOS 14, key_buf symbol 8 (outside the search's 32..127)
    return SDL_SCANCODE_BACKSPACE;
  }
  Fail("unknown key name " + t);
}

// One script event: a key event, or (text) one SDL_EVENT_TEXT_INPUT whose string is
// Case::texts[text].
struct ScriptEv {
  int frame = 0;
  bool down = false;
  bool repeat = false;
  SDL_Scancode sc = SDL_SCANCODE_UNKNOWN;
  int text = -1;
};

struct Case {
  std::string setup = "default";
  uint32_t boot_seed = 0;
  bool boot_given = false;
  std::vector<uint32_t> match_seeds;
  int frames = -1;
  std::string expect;
  std::vector<ScriptEv> events;
  std::deque<std::string> texts;  // SDL keeps the text pointers: alive for the whole run
  bool detail = false;
  std::string fs;
};

// `text` hex: 2..8 lowercase hex digits, an even count, no NUL byte (it would cut the C string).
std::string DecodeTextHex(std::string const& hex, std::string const& line) {
  if (hex.size() < 2 || hex.size() > 8 || hex.size() % 2 != 0) {
    Fail("bad text line: " + line);
  }
  auto nibble = [&](char c) -> int {
    if (c >= '0' && c <= '9') {
      return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
      return c - 'a' + 10;
    }
    Fail("bad text line: " + line);
  };
  std::string bytes;
  for (std::size_t i = 0; i < hex.size(); i += 2) {
    int const kB = (nibble(hex[i]) << 4) | nibble(hex[i + 1]);
    if (kB == 0) {
      Fail("bad text line (a NUL byte): " + line);
    }
    bytes += static_cast<char>(kB);
  }
  return bytes;
}

Case ParseCase(std::string const& path) {
  std::istringstream in(Slurp(path));
  Case c;
  std::string line;
  bool detail_given = false;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string key;
    if (!(ls >> key) || key[0] == '#') {
      continue;
    }
    std::string extra;
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
      ScriptEv k;
      std::string kind;
      std::string name;
      if (!(ls >> k.frame >> kind >> name) || k.frame < 0 ||
          (kind != "down" && kind != "up" && kind != "repeat")) {
        Fail("bad key line: " + line);
      }
      k.down = kind != "up";
      k.repeat = kind == "repeat";
      k.sc = ScancodeOf(name);
      c.events.push_back(k);
    } else if (key == "text") {
      ScriptEv t;
      std::string hex;
      if (!(ls >> t.frame >> hex) || t.frame < 0 || (ls >> extra)) {
        Fail("bad text line: " + line);
      }
      c.texts.push_back(DecodeTextHex(hex, line));
      t.text = static_cast<int>(c.texts.size()) - 1;
      c.events.push_back(t);
    } else if (key == "detail") {
      if (detail_given || (ls >> extra)) {
        Fail("bad detail line: " + line);
      }
      detail_given = true;
      c.detail = true;
    } else if (key == "fs") {
      if (!c.fs.empty() || !(ls >> c.fs) || (ls >> extra)) {
        Fail("bad fs line: " + line);
      }
    } else {
      Fail("bad script line: " + line);
    }
  }
  if (!c.boot_given || c.frames <= 0 || c.expect.empty()) {
    Fail("a shell script needs boot_seed, frames and expect");
  }
  if (!c.fs.empty() && c.setup != "default") {
    Fail("fs needs setup default (the fixture's liero.cfg is the setup)");
  }
  // Within a frame the file order is the SDL event order.
  std::stable_sort(c.events.begin(), c.events.end(),
                   [](ScriptEv const& a, ScriptEv const& b) { return a.frame < b.frame; });
  return c;
}

void Push(Case const& c, ScriptEv const& e) {
  SDL_Event ev{};
  if (e.text >= 0) {
    ev.type = SDL_EVENT_TEXT_INPUT;
    ev.text.text = c.texts[static_cast<std::size_t>(e.text)].c_str();
  } else {
    ev.type = e.down ? SDL_EVENT_KEY_DOWN : SDL_EVENT_KEY_UP;
    ev.key.scancode = e.sc;
    ev.key.key = SDL_GetKeyFromScancode(e.sc, SDL_KMOD_NONE, false);
    ev.key.mod = SDL_KMOD_NONE;
    ev.key.down = e.down;
    ev.key.repeat = e.repeat;
  }
  if (!SDL_PushEvent(&ev)) {
    Fail(std::string("SDL_PushEvent: ") + SDL_GetError());
  }
}

// A key-down the WeaponMenu search acts on (menu.cpp:14-20; TAB never clears the prefix).
bool IsSearchKey(ScriptEv const& e) {
  if (e.text >= 0 || !e.down) {
    return false;
  }
  SDL_Keycode const kSym = SDL_GetKeyFromScancode(e.sc, SDL_KMOD_NONE, false);
  return kSym >= 32 && kSym <= 127;
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
  if (dynamic_cast<WeaponMenuState*>(s) != nullptr) {
    return 'O';
  }
  if (dynamic_cast<InputStringState*>(s) != nullptr) {
    return 'I';
  }
  if (dynamic_cast<InfoBoxState*>(s) != nullptr) {
    return 'B';
  }
  Fail("a state 4½e-1 does not model is on the stack");
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

// The d line (plan D1): cur, ssel, cfg16, state8.
std::string DetailLine(int frame) {
  char cur = 0;
  if (gfx.cur_menu == &gfx.settings_menu) {
    cur = 'S';
  } else if (gfx.cur_menu == &gfx.main_menu) {
    cur = 'M';
  } else {
    Fail("frame " + std::to_string(frame) + ": cur_menu is neither the main nor the settings menu");
  }
  std::string state = "-";
  if (TopOf(gfx.state_stack.Top()) == 'G' && !gfx.controller->InWeaponSelection()) {
    state = Hex8(HashGameState(*gfx.controller->CurrentGame()));
  }
  return "d " + std::to_string(frame) + " " + cur + " " +
         std::to_string(gfx.settings_menu.Selection()) + " " +
         Hex16(FnvBytes(gfx.settings->ToToml())) + " " + state + "\n";
}

// A manifest <rel>: non-empty, forward slashes, no absolute path, no `\`, no `.`/`..`/empty part.
bool ValidRel(std::string const& rel) {
  if (rel.empty() || rel.front() == '/' || rel.contains('\\')) {
    return false;
  }
  std::size_t start = 0;
  while (true) {
    std::size_t const kEnd = rel.find('/', start);
    std::string const kPart =
        rel.substr(start, kEnd == std::string::npos ? std::string::npos : kEnd - start);
    if (kPart.empty() || kPart == "." || kPart == "..") {
      return false;
    }
    if (kEnd == std::string::npos) {
      return true;
    }
    start = kEnd + 1;
  }
}

// The fs fixture (plan §Formats): <temp>/oracle_shell_fs_<name>/{user,sys}, filled from the
// manifest with copies (no symlinks). Sources are repo-relative: call this from the repo root.
std::filesystem::path MakeFixture(std::string const& manifest, std::string const& name) {
  namespace fs = std::filesystem;
  fs::path const kRoot = fs::temp_directory_path() / ("oracle_shell_fs_" + name);
  fs::remove_all(kRoot);
  fs::create_directories(kRoot / "user");
  fs::create_directories(kRoot / "sys");
  std::istringstream in(Slurp(manifest));
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::vector<std::string> tok;
    std::string t;
    while (ls >> t && t[0] != '#') {
      tok.push_back(t);
    }
    if (tok.empty()) {
      continue;
    }
    bool const kDir = tok[0] == "dir" && tok.size() == 3;
    bool const kFile = tok[0] == "file" && tok.size() == 4;
    if ((!kDir && !kFile) || (tok[1] != "user" && tok[1] != "sys") || !ValidRel(tok[2])) {
      Fail("bad fs manifest line: " + line);
    }
    fs::path const kDest = kRoot / tok[1] / tok[2];
    if (kDir) {
      fs::create_directories(kDest);
      continue;
    }
    if (!fs::is_regular_file(tok[3])) {
      Fail("fs manifest source " + tok[3] + " is not a file");
    }
    if (fs::exists(kDest)) {
      Fail("fs manifest: " + tok[1] + "/" + tok[2] + " given twice");
    }
    fs::create_directories(kDest.parent_path());
    fs::copy_file(tok[3], kDest);
  }
  return kRoot;
}

// `file <rel> <fnv16>` for every regular file under user/, rel sorted bytewise.
std::string FileLines(std::filesystem::path const& user) {
  std::vector<std::pair<std::string, uint64_t>> files;
  for (auto const& e : std::filesystem::recursive_directory_iterator(user)) {
    if (e.is_regular_file()) {
      files.emplace_back(e.path().lexically_relative(user).generic_string(),
                         FnvBytes(Slurp(e.path().string())));
    }
  }
  std::ranges::sort(files);
  std::string out;
  for (auto const& [rel, h] : files) {
    out += "file " + rel + " " + Hex16(h) + "\n";
  }
  return out;
}

// Interventions 8 (and its fs form): a file level must open from the CWD, as level.cpp:401-411
// opens it; the dumper never lets GenerateFromSettings fall back to random silently.
void CheckLevelFile(std::string const& where) {
  if (gfx.settings->random_level) {
    return;
  }
  std::string path = gfx.settings->level_file;
  if (!path.contains('.')) {
    path += ".LEV";
  }
  try {
    (void)FsNode(path).ToReader();
  } catch (std::runtime_error const&) {
    Fail("the setup's level file " + path + " does not open from " + where);
  }
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
  if (kCase.fs.empty()) {
    auto settings = std::make_shared<Settings>();
    if (kCase.setup != "default") {
      try {
        settings->FromToml(Slurp(DirOf(kScript) + "/" + kCase.setup));
      } catch (std::exception const& e) {
        Fail("setup " + kCase.setup + ": " + e.what());
      }
    }
    gfx.settings = settings;
    gfx.settings_node = FsNode("data") / "Setups" / "liero.cfg";
  }
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");
  gfx.common = common;
  gfx.play_renderer.Init(kW, kH);
  gfx.play_renderer.LoadPalette(*common);
  gfx.single_screen_renderer.Init(640, 400);

  std::filesystem::path const kRepo = std::filesystem::current_path();
  std::filesystem::path fixture;
  if (!kCase.fs.empty()) {
    // The fs fixture: environment variables and CWD only; then the REAL paths::Resolve and the
    // REAL boot load / defaults save (gameEntry.cpp:30-58).
    std::string name = kScript.substr(kScript.find_last_of('/') + 1);
    if (name.ends_with("_script.txt")) {
      name.resize(name.size() - std::string_view("_script.txt").size());
    }
    if (name.starts_with("shell_")) {
      name.erase(0, std::string_view("shell_").size());
    }
    fixture = MakeFixture(DirOf(kScript) + "/" + kCase.fs, name);
    std::filesystem::current_path(fixture);
    if (setenv("OPENLIERO_TEST_USER_DIR", "user", 1) != 0 ||
        setenv("OPENLIERO_DATADIR", "sys", 1) != 0) {
      Fail("setenv failed");
    }
    std::string arg0 = "oracle_dump_shell";
    std::array<char*, 2> argv0 = {arg0.data(), nullptr};
    ResolvedPaths const kPaths = paths::Resolve(1, argv0.data());
    if (kPaths.user_config_node.FullPath() != "./user") {
      Fail("paths::Resolve picked " + kPaths.user_config_node.FullPath() +
           " as the user dir, not the fixture's ./user (a portable.txt next to the binary?)");
    }
    gfx.SetConfigNodes(kPaths.config_node, kPaths.user_config_node);
    if (!gfx.LoadSettings(gfx.GetConfigNode() / "Setups" / "liero.cfg")) {
      gfx.settings = std::make_shared<Settings>();
      gfx.SaveSettings(gfx.GetUserConfigNode() / "Setups" / "liero.cfg");
    }
  }
  gfx.settings->record_replays = false;  // intervention 6
  ColorMode const kMode = gfx.settings->modern_colors ? ColorMode::kModern : ColorMode::kClassic;
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

  if (kCase.fs.empty()) {
    std::filesystem::current_path("data/TC/openliero");  // intervention 8
    CheckLevelFile("data/TC/openliero");
  } else {
    CheckLevelFile("the fs fixture");
  }

  std::string out = "# oracle_dump_shell " + kScript +
                    " — the REAL C++ Gfx frame loop, headless (Step 4½d design §6.2)\n"
                    "# boot <presents> <presented16> <bmp16> <fade> <menu_cycles> <top> <sel>\n"
                    "# f <frame> <upd> <presents> <presented16> <bmp16> <fade> <menu_cycles> "
                    "<top> <sel> <sounds>\n";
  if (kCase.detail) {
    out += "# d <frame> <cur> <ssel> <cfg16> <state8|->\n";
  }
  if (!kCase.fs.empty()) {
    out += "# file <rel> <fnv16>\n";
  }
  gfx.rand.Seed(kCase.boot_seed);  // intervention 1
  gfx.last_frame = 0;              // intervention 5
  gfx.InitFrameStepping();
  out += "boot " + Presented(ppm_dir, "boot") + " " + Tail() + "\n";

  std::size_t next_seed = 0;
  std::size_t next_ev = 0;
  int end = kCase.frames;
  bool quit = false;
  // The search-gap check (plan D4): the WeaponMenuState of the current visit, and the frame and
  // SDL_GetTicks() of its latest search key.
  AppState const* search_menu = nullptr;
  int search_frame = -1;
  uint64_t search_ms = 0;
  for (int frame = 0; frame < kCase.frames; ++frame) {
    AppState* const kTopState = gfx.state_stack.Top();
    char const kTop = TopOf(kTopState);
    if (kTop == 'M' && next_seed < kCase.match_seeds.size()) {
      gfx.rand.Seed(kCase.match_seeds[next_seed]);  // intervention 2
    }
    char upd = kTop;
    if (kTop == 'G') {
      upd = gfx.controller->InWeaponSelection() ? 'W' : 'G';
    }
    if (kTop == 'O' && kTopState != search_menu) {
      search_menu = kTopState;
      search_frame = -1;
    } else if (kTop != 'O' && kTop != 'B') {
      search_menu = nullptr;
      search_frame = -1;
    }
    bool search_key = false;
    while (next_ev < kCase.events.size() && kCase.events[next_ev].frame == frame) {
      search_key = search_key || (kTop == 'O' && IsSearchKey(kCase.events[next_ev]));
      Push(kCase, kCase.events[next_ev++]);
    }
    if (search_key) {
      uint64_t const kNow = SDL_GetTicks();
      if (search_frame >= 0 && kNow - search_ms >= 1000) {
        Fail("search keys " + std::to_string(kNow - search_ms) + " ms apart on frames " +
             std::to_string(search_frame) + ", " + std::to_string(frame));
      }
      search_frame = frame;
      search_ms = kNow;
    }
    Controller const* const kBefore = gfx.controller.get();
    std::time_t const kT0 = std::time(nullptr);
    rec->played.clear();
    gfx.last_frame = 0;  // intervention 5
    bool const kGo = gfx.RunOneFrame();
    std::time_t const kT1 = std::time(nullptr);
    std::string const kAt = "frame " + std::to_string(frame) + ": ";
    if (gfx.controller.get() != kBefore) {
      // A NEW GAME made a controller: interventions 3 and 4.
      if (next_seed >= kCase.match_seeds.size()) {
        Fail(kAt + "a NEW GAME with no match_seed left");
      }
      if (gfx.settings->game_mode == Settings::kGmHoldazone) {
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
    } else if (kTop == 'M' && kBefore != nullptr && TopOf(gfx.state_stack.Top()) == 'G' &&
               gfx.settings->game_mode == Settings::kGmHoldazone) {
      Fail(kAt + "a RESUME into Holdazone (unported in Rust)");
    }
    std::string sounds;
    for (int const kId : rec->played) {
      sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
    }
    std::array<char, 16> name{};
    std::snprintf(name.data(), name.size(), "f_%04d", frame);
    out += "f " + std::to_string(frame) + " " + upd + " " + Presented(ppm_dir, name.data()) + " " +
           Tail() + " " + (sounds.empty() ? "-" : sounds) + "\n";
    if (kCase.detail) {
      out += DetailLine(frame);
    }
    if (!kGo) {
      end = frame;
      quit = true;
      break;
    }
  }
  if (next_ev != kCase.events.size()) {
    Fail("script events scheduled after the run ended");
  }
  if (next_seed != kCase.match_seeds.size()) {
    Fail("unused match seeds");
  }
  if ((kCase.expect == "quit") != quit) {
    Fail("expected to end by " + kCase.expect);
  }
  out += "end " + std::to_string(end) + " " + (quit ? "quit" : "frames") + "\n";
  if (!kCase.fs.empty()) {
    if (quit) {
      // Intervention 9: the exit save, gameEntry.cpp:78 verbatim.
      gfx.settings->save(gfx.GetUserConfigNode() / "Setups" / "liero.cfg", gfx.rand);
    }
    out += FileLines(fixture / "user");
  }

  std::ofstream f(kOut, std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + kOut);
  }
  if (!kCase.fs.empty()) {
    std::filesystem::current_path(kRepo);
    std::filesystem::remove_all(fixture);
  }
  std::printf("oracle_dump_shell: %d frames, %zu NEW GAMEs, end %s\n", quit ? end + 1 : end,
              next_seed, quit ? "quit" : "frames");
  gfx.controller.reset();
  g_sound_player = nullptr;
  return 0;
}
