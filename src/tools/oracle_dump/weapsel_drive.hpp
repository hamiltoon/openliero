// Step 4½c — the weapon-selection phase driver shared by oracle_dump_weapsel (weapsel_dump.cpp:
// the weapsel_* goldens) and oracle_dump_sim_physics's `settings` path (sim_physics_dump.cpp: the
// sim_slice4_5c_* continuation goldens). Design:
// docs/superpowers/specs/2026-09-25-liero-rs-step4.5-slice4.5c-weapon-selection-design.md §6.2.
//
// The REAL WeaponSelection (weapsel.cpp) is constructed, stepped and finalized. Only
// LocalController's input plumbing is replicated, because a scenario holds SAMPLED per-frame
// words (as the Rust sampler does), not key events:
//   * a changed bit is one LocalController::OnKey event (localController.cpp:58-80): the clean
//     and the control bit follow it, then the Dig chord block;
//   * then a VERBATIM copy of the weapsel key-repeat loop (localController.cpp:128-148);
//   * then the REAL WeaponSelection::ProcessFrame (weapsel.cpp:219-350).
// oracle_dump_weapsel proves this replica against a real LocalController (its self-check).
#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <string>
#include <vector>

#include "game.hpp"
#include "math/rect.hpp"
#include "mixer/player.hpp"
#include "rand.hpp"
#include "viewport.hpp"
#include "weapsel.hpp"
#include "worm.hpp"

namespace weapsel_drive {

// frame -> the sampled 7-bit word per worm (a scenario's sparse `weapsel` lines).
using Script = std::map<int, std::array<uint32_t, 2>>;

constexpr std::size_t kRepeatBits = 7;  // localController.cpp:130
constexpr int kKeyRepeatInitial = 12;   // localController.hpp:44
constexpr int kKeyRepeatInterval = 3;   // localController.hpp:45
constexpr uint64_t kMaxDraws = 1000000;

[[noreturn]] inline void Fail(std::string const& what) {
  std::fprintf(stderr, "weapsel: %s\n", what.c_str());
  std::exit(1);
}

// Logs the sample id of every Play that reaches the mixer (SoundPlayer::Play already dropped a
// negative id, mixer/player.hpp:15-22).
struct RecordingSoundPlayer : SoundPlayer {
  bool IsPlaying(void* /*id*/) override { return false; }
  void Stop(void* /*id*/) override {}
  std::vector<int> played;

 protected:
  void PlayImpl(int sound, void* /*id*/, int /*loops*/) override { played.push_back(sound); }
};

// RNG steps from `before` to `after`. C++ Rand has no counter: step a copy of `before` until its
// mt19937 state equals `after`'s (rand.hpp:15), then require `last` to agree too.
inline uint64_t CountDraws(Rand const& before, Rand const& after) {
  Rand probe = before;
  uint64_t n = 0;
  while (probe.engine != after.engine) {
    probe();
    if (++n > kMaxDraws) {
      Fail("more than 10^6 RNG draws in one step");
    }
  }
  if (probe.last != after.last) {
    Fail("draw count: the engines agree but rand.last does not");
  }
  return n;
}

// The next raw value `r` would return, without advancing it.
inline uint32_t PeekNext(Rand const& r) {
  Rand probe = r;
  return probe();
}

class Driver {
 public:
  // Registers the LocalController viewports (localController.cpp:47-48) — WeaponSelection maps
  // menus[i] to game.viewports[i]->worm_idx (weapsel.cpp:44-47) — then runs the REAL
  // constructor (it draws game.rand).
  explicit Driver(Game& game) : game_(game) {
    if (game_.worms.size() != 2) {
      Fail("the phase needs exactly two worms");
    }
    viewports_[0] = std::make_unique<Viewport>(Rect(0, 0, 158, 158), 0);
    viewports_[1] = std::make_unique<Viewport>(Rect(160, 0, 158 + 160, 158), 1);
    for (auto const& vp : viewports_) {
      game_.AddViewport(vp.get());
    }
    ws_ = std::make_unique<WeaponSelection>(game_);
  }
  Driver(Driver const&) = delete;
  Driver& operator=(Driver const&) = delete;
  Driver(Driver&&) = delete;
  Driver& operator=(Driver&&) = delete;
  // The match runs with the viewports unregistered (the dumper's settings path has none).
  ~Driver() { game_.ClearViewports(); }

  // One LocalController::Process weapsel frame (localController.cpp:124-152) on sampled words.
  bool Frame(std::array<uint32_t, 2> const& words) {
    for (std::size_t wi = 0; wi < 2; ++wi) {
      Worm& worm = *game_.worms[wi];
      uint32_t const kNow = words[wi] & 0x7FU;
      uint32_t const kChanged = kNow ^ prev_[wi];
      for (std::size_t bit = 0; bit < kRepeatBits; ++bit) {
        if (((kChanged >> bit) & 1U) == 0) {
          continue;
        }
        // LocalController::OnKey for this key event (localController.cpp:58-80). Every sampled
        // bit is a real control (< Worm::kMaxControl), so SetControlState always runs.
        bool const kDown = ((kNow >> bit) & 1U) != 0;
        auto const kControl = static_cast<Worm::Control>(bit);
        worm.clean_control_states.Set(kControl, kDown);
        worm.SetControlState(kControl, kDown);
        if (worm.clean_control_states[WormSettings::kDig]) {
          worm.Press(Worm::kLeft);
          worm.Press(Worm::kRight);
        } else {
          if (!worm.clean_control_states[Worm::kLeft]) {
            worm.Release(Worm::kLeft);
          }
          if (!worm.clean_control_states[Worm::kRight]) {
            worm.Release(Worm::kRight);
          }
        }
      }
      prev_[wi] = kNow;
    }
    // localController.cpp:128-148, verbatim.
    for (std::size_t wi = 0; wi < game_.worms.size(); ++wi) {
      Worm& worm = *game_.worms[wi];
      for (std::size_t bit = 0; bit < kRepeatBits; ++bit) {
        bool const kHeld = worm.clean_control_states[bit];
        if (kHeld) {
          if (!worm.control_states[bit]) {
            ++held_[wi][bit];
            if (held_[wi][bit] >= kKeyRepeatInitial &&
                (held_[wi][bit] - kKeyRepeatInitial) % kKeyRepeatInterval == 0) {
              worm.Press(static_cast<Worm::Control>(bit));
            }
          } else {
            held_[wi][bit] = 0;
          }
        } else {
          held_[wi][bit] = 0;
        }
      }
    }
    return ws_->ProcessFrame();
  }

  void Finalize() { ws_->Finalize(); }
  WeaponSelection const& Ws() const { return *ws_; }
  // For WeaponSelection::Draw (non-const: it caches the frozen background).
  WeaponSelection& MutableWs() { return *ws_; }
  std::array<uint16_t, kRepeatBits> const& Held(std::size_t wi) const { return held_[wi]; }

 private:
  Game& game_;
  std::array<std::unique_ptr<Viewport>, 2> viewports_;
  std::unique_ptr<WeaponSelection> ws_;
  std::array<uint32_t, 2> prev_{};
  std::array<std::array<uint16_t, kRepeatBits>, 2> held_{};
};

// Runs the whole phase: the constructor, frames 0..end (an absent frame is {0, 0}; words are
// masked to 7 bits), then Finalize. `on_init(driver)` runs after the constructor and
// `on_frame(driver, frame, words, done)` after each ProcessFrame. The end-frame invariant (design
// §6.1): ProcessFrame must return true on the last `weapsel` frame and on no earlier one.
template <typename OnInit, typename OnFrame>
void Run(Game& game, Script const& script, OnInit const& on_init, OnFrame const& on_frame) {
  if (script.empty()) {
    Fail("no weapsel lines");
  }
  int const kEnd = script.rbegin()->first;
  Driver driver(game);
  on_init(driver);
  for (int frame = 0; frame <= kEnd; ++frame) {
    std::array<uint32_t, 2> words{0, 0};
    auto const kIt = script.find(frame);
    if (kIt != script.end()) {
      words = {kIt->second[0] & 0x7FU, kIt->second[1] & 0x7FU};
    }
    bool const kDone = driver.Frame(words);
    on_frame(driver, frame, words, kDone);
    if (kDone && frame != kEnd) {
      Fail("the phase ended on frame " + std::to_string(frame) +
           ", before the last weapsel line (" + std::to_string(kEnd) + ")");
    }
    if (!kDone && frame == kEnd) {
      Fail("the phase is not over on the last weapsel frame " + std::to_string(kEnd));
    }
  }
  driver.Finalize();
}

}  // namespace weapsel_drive
