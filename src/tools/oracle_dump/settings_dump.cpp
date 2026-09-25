// Generates the C++ side of the Rust settings-persistence byte gate (Step 4½, slice 4½a-2;
// rust/oracle-tests/tests/settings_toml_golden.rs). For every corpus entry it runs the REAL C++
// persistence path - Settings::load + Settings::save (settings.cpp:62-90, :158-163, i.e.
// FromToml/ToToml), WormSettings::LoadProfile + SaveProfile (worm.cpp:60-95) - and the REAL
// Settings::UpdateHash / WormSettings::UpdateHash (settings.cpp:92-101, worm.cpp:38-43), and
// writes the bytes C++ saves as goldens.
//
// Usage (run from the repo root): oracle_dump_settings <corpus.txt> <out-dir>
// corpus.txt: one entry per line; blank lines and '#' lines are skipped:
//   default-setup <id>        Settings() saved                  -> <id>.cfg, <id>.gameplay.toml
//   default-profile <id>      WormSettings() saved              -> <id>.toml
//   setup <id> <path>         Settings::load(path), then save   -> <id>.cfg, <id>.gameplay.toml
//   profile <id> <path>       LoadProfile(path) over WormSettings(), then SaveProfile -> <id>.toml
// <path> is the rest of the line (it may contain spaces), relative to the repo root.
// <out-dir>/hashes.txt: "xxh3-empty - <XXH3_64 of zero bytes>", then "<kind> <id> <hash>" per
// entry in corpus order (%016llx): Settings::UpdateHash() for setups (whose input is exactly
// <id>.gameplay.toml), WormSettings::UpdateHash() for profiles (whose input is <id>.toml).
//
// Self-checks (each exits 1): every input parses (LoadProfile only warns on a parse error, so
// profiles are pre-parsed with toml::parse); each UpdateHash() equals XXH3_64 of the bytes
// written; and C++ load + save of every file it wrote reproduces that file byte for byte, so
// "a C++-saved file round-trips through Rust" (G5b) is a meaningful check. Built via
// OPENLIERO_BUILD_ORACLE_DUMP (see rust/oracle-tests/gen_settings_golden.sh). Not part of the
// default build.
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <sstream>
#include <string>

#include "filesystem.hpp"
#include "rand.hpp"
#include "settings.hpp"
#include "worm.hpp"

#include <xxhash.h>
#include <serialization/cereal_types.hpp>
#include <serialization/toml_archive.hpp>

namespace {

[[noreturn]] void Fail(std::string const& what) {
  std::fprintf(stderr, "oracle_dump_settings: %s\n", what.c_str());
  std::exit(1);
}

std::string Slurp(std::string const& path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    Fail("cannot open " + path);
  }
  return {std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>()};
}

void Spit(std::string const& path, std::string const& bytes) {
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  f.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
  if (!f) {
    Fail("cannot write " + path);
  }
}

uint64_t Xxh3(std::string const& bytes) { return XXH3_64bits(bytes.data(), bytes.size()); }

// The SerializeGameplay TOML exactly as Settings::UpdateHash builds it (settings.cpp:93-98).
std::string GameplayToml(Settings& s) {
  std::ostringstream ss;
  {
    cereal::TomlOutputArchive ar(ss);
    SerializeGameplay(ar, s);
  }
  return ss.str();
}

void EmitHash(std::FILE* hashes, std::string const& kind, std::string const& id, uint64_t hash) {
  std::fprintf(hashes, "%s %s %016" PRIx64 "\n", kind.c_str(), id.c_str(), hash);
}

// Settings::save of `s`, its gameplay bytes and UpdateHash, then the idempotence check.
void EmitSetup(std::FILE* hashes, std::string const& kind, std::string const& id, Settings& s,
               std::string const& dir) {
  Rand rand;
  std::string const kCfgPath = dir + "/" + id + ".cfg";
  s.save(FsNode(kCfgPath), rand);
  std::string const kSaved = Slurp(kCfgPath);
  std::string const kGameplay = GameplayToml(s);
  Spit(dir + "/" + id + ".gameplay.toml", kGameplay);
  uint64_t const kHash = s.UpdateHash();
  if (kHash != Xxh3(kGameplay)) {
    Fail("Settings::UpdateHash() is not XXH3_64 of the gameplay TOML for " + id);
  }
  Settings again;
  if (!again.load(FsNode(kCfgPath), rand) || again.ToToml() != kSaved) {
    Fail("C++ load + save does not reproduce its own output " + kCfgPath);
  }
  EmitHash(hashes, kind, id, kHash);
}

// WormSettings::SaveProfile of `ws` and its UpdateHash, then the idempotence check.
void EmitProfile(std::FILE* hashes, std::string const& kind, std::string const& id,
                 WormSettings& ws, std::string const& dir) {
  std::string const kPath = dir + "/" + id + ".toml";
  ws.SaveProfile(FsNode(kPath));
  std::string const kSaved = Slurp(kPath);
  uint64_t const kHash = ws.UpdateHash();
  if (kHash != Xxh3(kSaved)) {
    Fail("WormSettings::UpdateHash() is not XXH3_64 of the saved profile for " + id);
  }
  WormSettings again;
  again.LoadProfile(FsNode(kPath));
  if (again.ToToml() != kSaved) {
    Fail("C++ LoadProfile + SaveProfile does not reproduce its own output " + kPath);
  }
  EmitHash(hashes, kind, id, kHash);
}

// LoadProfile only logs a parse error (worm.cpp:90-92); the oracle must not fall back silently.
void RequireParses(std::string const& path) {
  try {
    (void)toml::parse(Slurp(path), path);
  } catch (toml::parse_error const& e) {
    Fail(path + ": " + e.what());
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc != 3) {
    std::fprintf(stderr, "usage: oracle_dump_settings <corpus.txt> <out-dir>\n");
    return 1;
  }
  std::string const kDir = argv[2];
  std::istringstream corpus(Slurp(argv[1]));
  // The "e" (O_CLOEXEC) mode is a glibc extension; this tool also builds on macOS and never execs.
  // NOLINTNEXTLINE(android-cloexec-fopen)
  std::FILE* hashes = std::fopen((kDir + "/hashes.txt").c_str(), "w");
  if (!hashes) {
    Fail("cannot write " + kDir + "/hashes.txt");
  }
  std::fprintf(hashes, "xxh3-empty - %016" PRIx64 "\n", Xxh3(std::string()));
  int entries = 0;
  std::string line;
  while (std::getline(corpus, line)) {
    if (line.empty() || line[0] == '#') {
      continue;
    }
    std::istringstream ls(line);
    std::string kind;
    std::string id;
    std::string path;
    ls >> kind >> id;
    std::getline(ls >> std::ws, path);
    if (id.empty()) {
      Fail("bad corpus line: " + line);
    }
    if (kind == "default-setup" && path.empty()) {
      Settings s;
      EmitSetup(hashes, kind, id, s, kDir);
    } else if (kind == "default-profile" && path.empty()) {
      WormSettings ws;
      EmitProfile(hashes, kind, id, ws, kDir);
    } else if (kind == "setup" && !path.empty()) {
      Rand rand;
      Settings s;
      if (!s.load(FsNode(path), rand)) {
        Fail("Settings::load failed on " + path);
      }
      EmitSetup(hashes, kind, id, s, kDir);
    } else if (kind == "profile" && !path.empty()) {
      RequireParses(path);
      WormSettings ws;
      ws.LoadProfile(FsNode(path));
      EmitProfile(hashes, kind, id, ws, kDir);
    } else {
      Fail("bad corpus line: " + line);
    }
    ++entries;
  }
  std::fclose(hashes);
  std::printf("oracle_dump_settings: %d entries\n", entries);
  return 0;
}
