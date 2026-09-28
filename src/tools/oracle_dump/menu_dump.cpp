// Generates the C++ side of the Rust menu-widget gate G1 (Step 4½, slice 4½d; design §6.1;
// rust/oracle-tests/tests/menu_widget_golden.rs). A script drives either
//   * a widget menu: a REAL Menu subclass whose GetItemBehavior maps item ids to the REAL
//     Integer/Time/BooleanSwitch/Enum/ArrayEnum behaviors over variables this dumper owns, or
//   * the REAL SettingsMenu (gfx.settings_menu after the REAL Gfx::LoadMenus) over the REAL
//     Settings::FromToml of a setup sidecar, with gfx.settings_node = data/Setups/liero.cfg,
// through a list of ops, and writes one line per op:
//   <n> <op> <sel> <top> <bottom> <vis> <shown> <prefix> <vals> <bound> <sounds> <ret> <push>
//   <hash>
// (grammar and fields: docs/superpowers/plans/2026-09-26-liero-rs-step4.5-slice4.5d-plan.md,
// Task 4). Sounds are logged by a RecordingSoundPlayer installed as g_sound_player; an
// InputStringState an IntegerBehavior::OnEnter pushes is counted and popped at once; `draw`
// fills play_renderer.bmp with colour 0, draws through the plain TC palette, and hashes the
// identity pixels (render/src/hash.rs). `sleep_ms` really sleeps (SDL_Delay) because OnKeys reads
// SDL_GetTicks; only 0, 1..1000 and >= 1600 are allowed, so scheduler jitter cannot flip the
// 1500 ms test. Usage (from the repo root):
//   oracle_dump_menu <script.txt> <out.txt> [--ppm-dir <dir>]
// Built via OPENLIERO_BUILD_ORACLE_DUMP (rust/oracle-tests/gen_menu_golden.sh). Not part of the
// default build.
#include <SDL3/SDL.h>

#include <array>
#include <cinttypes>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "common.hpp"
#include "filesystem.hpp"
#include "gfx.hpp"
#include "gfx/bitmap.hpp"
#include "gfx/blit.hpp"
#include "keys.hpp"
#include "math.hpp"
#include "menu/arrayEnumBehavior.hpp"
#include "menu/booleanSwitchBehavior.hpp"
#include "menu/enumBehavior.hpp"
#include "menu/integerBehavior.hpp"
#include "menu/itemBehavior.hpp"
#include "menu/menu.hpp"
#include "menu/menuItem.hpp"
#include "menu/timeBehavior.hpp"
#include "mixer/player.hpp"
#include "settings.hpp"
#include "weapsel_drive.hpp"

namespace {

using weapsel_drive::Fail;
using weapsel_drive::RecordingSoundPlayer;

constexpr uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr uint64_t kFnvPrime = 1099511628211ULL;
constexpr int kIdentityFade = 33;

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

void WritePpm(std::string const& path, Bitmap const& bmp) {
  std::string data = "P6\n" + std::to_string(bmp.w) + " " + std::to_string(bmp.h) + "\n255\n";
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

std::string DirOf(std::string const& path) {
  std::size_t const kSlash = path.find_last_of('/');
  return kSlash == std::string::npos ? std::string(".") : path.substr(0, kSlash);
}

// One bound variable and its behavior's parameters.
struct Bind {
  std::string kind;  // integer | time | bool | enum | array
  int i = 0;
  uint32_t u = 0;
  bool b = false;
  int min = 0;
  int max = 0;
  int step = 1;
  int interval = 5;
  int div = 1;
  bool pct = false;
  bool entry = true;
  bool frames = false;
  bool broken = false;
  std::string arr;  // gamemodes | onoff

  std::string Value() const {
    if (kind == "bool") {
      return b ? "1" : "0";
    }
    if (kind == "enum" || kind == "array") {
      return std::to_string(u);
    }
    return std::to_string(i);
  }
};

// A widget menu: GetItemBehavior builds the REAL behavior over the bound variable, as C++ menus
// do (menu.hpp:69-97).
struct ScriptMenu : Menu {
  ScriptMenu(int x, int y, bool centered, std::map<int, Bind>* binds)
      : Menu(x, y, centered), binds_(binds) {}

  ItemBehavior* GetItemBehavior(Common& common, MenuItem& item) override {
    auto it = binds_->find(item.id);
    if (it == binds_->end()) {
      return Menu::GetItemBehavior(common, item);
    }
    Bind& b = it->second;
    if (b.kind == "integer") {
      auto* r = new IntegerBehavior(common, b.i, b.min, b.max, b.step, b.pct);
      r->scroll_interval = b.interval;
      r->display_div = b.div;
      r->allow_entry = b.entry;
      return r;
    }
    if (b.kind == "time") {
      return new TimeBehavior(common, b.i, b.min, b.max, b.step, b.frames);
    }
    if (b.kind == "bool") {
      return new BooleanSwitchBehavior(common, b.b);
    }
    if (b.kind == "enum") {
      return new EnumBehavior(common, b.u, static_cast<uint32_t>(b.min),
                              static_cast<uint32_t>(b.max), b.broken);
    }
    if (b.arr == "gamemodes") {
      return new ArrayEnumBehavior(common, b.u, common.texts.game_modes, b.broken);
    }
    return new ArrayEnumBehavior(common, b.u, common.texts.onoff, b.broken);
  }

 private:
  std::map<int, Bind>* binds_;
};

SDL_Scancode ScancodeOf(std::string const& t) {
  if (t.size() == 1 && t[0] >= 'a' && t[0] <= 'z') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_A + (t[0] - 'a'));
  }
  if (t.size() == 1 && t[0] >= '1' && t[0] <= '9') {
    return static_cast<SDL_Scancode>(SDL_SCANCODE_1 + (t[0] - '1'));
  }
  if (t == "0") {
    return SDL_SCANCODE_0;
  }
  if (t == "SPACE") {
    return SDL_SCANCODE_SPACE;
  }
  if (t == "TAB") {
    return SDL_SCANCODE_TAB;
  }
  if (t == "MINUS") {
    return SDL_SCANCODE_MINUS;
  }
  if (t == "UP") {
    return SDL_SCANCODE_UP;
  }
  Fail("unknown key token " + t);
}

std::string Token(std::string s) {
  if (s.empty()) {
    return "-";
  }
  for (char& c : s) {
    if (c == ' ') {
      c = '_';
    }
  }
  return s;
}

std::string Line(int n, std::string const& op, Menu& m, std::map<int, Bind> const& binds,
                 RecordingSoundPlayer& rec, std::string const& ret, std::size_t push,
                 std::string const& hash) {
  std::string shown;
  std::string vals;
  for (std::size_t i = 0; i < m.items.size(); ++i) {
    MenuItem const& it = m.items[i];
    shown += it.visible ? '1' : '0';
    if (i != 0) {
      vals += '|';
    }
    vals += it.has_value ? Token(it.value) : ".";
  }
  std::string bound;
  for (auto const& [id, b] : binds) {
    bound += (bound.empty() ? "" : ",") + b.Value();
  }
  std::string sounds;
  for (int const kId : rec.played) {
    sounds += (sounds.empty() ? "" : ",") + std::to_string(kId);
  }
  rec.played.clear();
  return std::to_string(n) + " " + op + " " + std::to_string(m.Selection()) + " " +
         std::to_string(m.top_item) + " " + std::to_string(m.bottom_item) + " " +
         std::to_string(m.visible_item_count) + " " + (shown.empty() ? "-" : shown) + " " +
         Token(m.search_prefix) + " " + (vals.empty() ? "-" : vals) + " " +
         (bound.empty() ? "-" : bound) + " " + (sounds.empty() ? "-" : sounds) + " " + ret + " " +
         std::to_string(push) + " " + hash + "\n";
}

}  // namespace

int main(int argc, char** argv) {
  std::vector<std::string> const kArgs(argv + 1, argv + argc);
  std::vector<std::string> positional;
  std::string ppm_dir;
  for (std::size_t i = 0; i < kArgs.size(); ++i) {
    if (kArgs[i] == "--ppm-dir" && i + 1 < kArgs.size()) {
      ppm_dir = kArgs[++i];
    } else if (kArgs[i].starts_with("--")) {
      Fail("unknown or incomplete option " + kArgs[i]);
    } else {
      positional.push_back(kArgs[i]);
    }
  }
  if (positional.size() != 2) {
    Fail("usage: oracle_dump_menu <script.txt> <out.txt> [--ppm-dir <dir>]");
  }
  std::string const& script_path = positional[0];

  // OnKeys reads SDL_GetKeyFromScancode (menu.cpp:17) and SDL_GetTicks (:21).
  if (!SDL_Init(SDL_INIT_EVENTS)) {
    Fail(std::string("SDL_Init: ") + SDL_GetError());
  }
  if (SDL_GetKeyFromScancode(SDL_SCANCODE_A, SDL_KMOD_NONE, false) != SDLK_A) {
    Fail("SDL_GetKeyFromScancode(A) is not 'a' without a video subsystem");
  }
  InitKeys();
  PrecomputeTables();
  auto common = std::make_shared<Common>();
  common->load(FsNode("data") / "TC" / "openliero");
  gfx.common = common;
  gfx.play_renderer.Init(320, 200);
  gfx.play_renderer.LoadPalette(*common);  // the plain TC palette (renderer.cpp:14-19)
  auto rec = std::make_shared<RecordingSoundPlayer>();
  gfx.sound_player = rec;
  g_sound_player = rec.get();

  std::istringstream in(Slurp(script_path));
  std::map<int, Bind> binds;
  std::unique_ptr<ScriptMenu> own;
  Menu* menu = nullptr;
  std::string out = "# oracle_dump_menu " + script_path +
                    " — the REAL C++ Menu / behaviors / SettingsMenu (Step 4½d design §6.1)\n"
                    "# <n> <op> <sel> <top> <bottom> <vis> <shown> <prefix> <vals> <bound> "
                    "<sounds> <ret> <push> <hash>\n";
  int n = 0;
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::string op;
    if (!(ls >> op) || op[0] == '#') {
      continue;
    }
    if (op == "menu" || op == "settings") {
      if (menu != nullptr) {
        Fail("a second menu/settings directive: " + line);
      }
      if (op == "menu") {
        int x = 0;
        int y = 0;
        int height = 0;
        int voff = 0;
        int centered = 0;
        if (!(ls >> x >> y >> height >> voff >> centered)) {
          Fail("bad menu line: " + line);
        }
        own = std::make_unique<ScriptMenu>(x, y, centered != 0, &binds);
        own->height = height;
        own->value_offset_x = voff;
        menu = own.get();
      } else {
        std::string setup;
        ls >> setup;
        auto settings = std::make_shared<Settings>();
        if (setup != "default") {
          try {
            settings->FromToml(Slurp(DirOf(script_path) + "/" + setup));
          } catch (std::exception const& e) {
            Fail("settings " + setup + ": " + e.what());
          }
        }
        gfx.settings = settings;
        gfx.settings_node = FsNode("data") / "Setups" / "liero.cfg";  // OptionsSave: "liero"
        gfx.LoadMenus();
        menu = &gfx.settings_menu;
      }
      continue;
    }
    if (menu == nullptr) {
      Fail("the first directive must be menu or settings: " + line);
    }
    if (op == "item" || op == "space" || op == "bind") {
      if (!own) {
        std::string msg = op;
        msg += " is for widget scripts: ";
        msg += line;
        Fail(msg);
      }
      if (op == "space") {
        own->AddItem(MenuItem::Space());
      } else if (op == "item") {
        int id = 0;
        int color = 0;
        int dis = 0;
        int visible = 0;
        int selectable = 0;
        if (!(ls >> id >> color >> dis >> visible >> selectable)) {
          Fail("bad item line: " + line);
        }
        std::string label;
        std::getline(ls >> std::ws, label);
        if (label == "\"\"") {
          label.clear();
        }
        MenuItem item(static_cast<PalIdx>(color), static_cast<PalIdx>(dis), label, id);
        item.visible = visible != 0;
        item.selectable = selectable != 0;
        own->AddItem(item);
      } else {
        int id = 0;
        Bind b;
        ls >> id >> b.kind;
        bool ok = true;
        if (b.kind == "integer") {
          int pct = 0;
          int entry = 0;
          ok = static_cast<bool>(ls >> b.i >> b.min >> b.max >> b.step >> pct >> b.interval >>
                                 b.div >> entry);
          b.pct = pct != 0;
          b.entry = entry != 0;
        } else if (b.kind == "time") {
          int frames = 0;
          ok = static_cast<bool>(ls >> b.i >> b.min >> b.max >> b.step >> frames);
          b.frames = frames != 0;
        } else if (b.kind == "bool") {
          int v = 0;
          ok = static_cast<bool>(ls >> v);
          b.b = v != 0;
        } else if (b.kind == "enum") {
          int broken = 0;
          ok = static_cast<bool>(ls >> b.u >> b.min >> b.max >> broken);
          b.broken = broken != 0;
        } else if (b.kind == "array") {
          int broken = 0;
          ok = static_cast<bool>(ls >> b.u >> b.arr >> broken) &&
               (b.arr == "gamemodes" || b.arr == "onoff");
          b.broken = broken != 0;
        } else {
          ok = false;
        }
        if (!ok || !binds.emplace(id, b).second) {
          Fail("bad or duplicate bind line: " + line);
        }
      }
      continue;
    }

    std::string ret = "-";
    std::size_t push = 0;
    std::string hash = "-";
    int a = 0;
    int b = 0;
    int c = 0;
    if (op == "move" && (ls >> a)) {
      menu->Movement(a);
    } else if (op == "page" && (ls >> a)) {
      menu->MovementPage(a);
    } else if (op == "scroll" && (ls >> a)) {
      menu->Scroll(a);
    } else if (op == "set_height" && (ls >> a)) {
      menu->SetHeight(a);
    } else if (op == "visible" && (ls >> a >> b)) {
      menu->SetVisibility(a, b != 0);
    } else if (op == "move_to" && (ls >> a)) {
      menu->MoveTo(a);
    } else if (op == "move_to_id" && (ls >> a)) {
      menu->MoveToId(a);
    } else if (op == "first_visible") {
      menu->MoveToFirstVisible();
    } else if (op == "cycles") {
      std::string v;
      ls >> v;
      gfx.menu_cycles = static_cast<unsigned>(std::strtoul(v.c_str(), nullptr, 10));
    } else if (op == "left" || op == "right") {
      ret = menu->OnLeftRight(*common, op == "left" ? -1 : 1) ? "1" : "0";
    } else if (op == "enter") {
      std::size_t const kBefore = gfx.state_stack.Size();
      ret = std::to_string(menu->OnEnter(*common));
      push = gfx.state_stack.Size() - kBefore;
      while (gfx.state_stack.Size() > kBefore) {
        gfx.state_stack.Pop();
      }
    } else if (op == "keys" || op == "keys_contains") {
      std::vector<SDL_Scancode> keys;
      std::string t;
      while (ls >> t) {
        keys.push_back(ScancodeOf(t));
      }
      menu->OnKeys(keys.data(), keys.data() + keys.size(), op == "keys_contains");
    } else if (op == "sleep_ms" && (ls >> a)) {
      if (a < 0 || (a > 1000 && a < 1600)) {
        Fail("sleep_ms must be 0, 1..1000 or >= 1600 (the 1500 ms search timeout): " + line);
      }
      SDL_Delay(static_cast<uint32_t>(a));
    } else if (op == "update_items") {
      menu->UpdateItems(*common);
    } else if (op == "draw" && (ls >> a >> b >> c)) {
      Fill(gfx.play_renderer.bmp, 0);
      menu->Draw(*common, gfx.play_renderer, a != 0, b, c != 0);
      std::array<char, 24> buf{};
      std::snprintf(buf.data(), buf.size(), "%016" PRIx64,
                    HashFrame(gfx.play_renderer.bmp, kIdentityFade));
      hash = buf.data();
      if (!ppm_dir.empty()) {
        std::array<char, 32> name{};
        std::snprintf(name.data(), name.size(), "/menu_%04d.ppm", n);
        WritePpm(ppm_dir + name.data(), gfx.play_renderer.bmp);
      }
    } else {
      Fail("bad op line: " + line);
    }
    out += Line(n, op, *menu, binds, *rec, ret, push, hash);
    ++n;
  }

  std::ofstream f(positional[1], std::ios::binary | std::ios::trunc);
  f << out;
  if (!f) {
    Fail("cannot write " + positional[1]);
  }
  std::printf("oracle_dump_menu: %d ops\n", n);
  g_sound_player = nullptr;
  SDL_Quit();
  return 0;
}
