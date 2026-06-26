// Generates golden vectors from the C++ oracle for the Rust differential tests.
//
// This runs the *existing* C++ engine functions (Itof/Ftoi, IVec2, VectorLength,
// the cossin table, Rand) and writes their results to text files. The Rust port
// in rust/sim-core is then tested against these files: if Rust reproduces every
// value bit-for-bit, the port is proven correct. The input lists below MUST stay
// identical to the Rust tests' lists.
//
// Built standalone (see rust/oracle-tests/gen_golden.sh) — not part of CMake.
#include <cstdint>
#include <cstdio>
#include <string>

#include "math.hpp"
#include "rand.hpp"

namespace {

// MUST stay identical to the Rust tests' lists.
int const kFixedInputs[] = {-2000000, -65537, -65536, -100, -1, 0,
                            1,        100,     65535,  65536, 65537, 2000000};

struct VecCase {
  int ax, ay, bx, by, s;
};
VecCase const kVecCases[] = {
    {0, 0, 0, 0, 1},        {100, -50, 7, 9, 3},     {-65536, 65536, 100, -100, 100},
    {123456, -789012, -3, 5, 7}, {2000000, -2000000, 1, 1, 2}};

struct SqrtCase {
  int x, y;
};
SqrtCase const kSqrtCases[] = {{0, 0},   {3, 4},     {100, 0},  {0, 255},
                               {1000, 1000}, {-1234, 5678}, {32767, 32767}};

void DumpFixed(std::FILE* f) {
  for (int v : kFixedInputs) {
    std::fprintf(f, "%d\n", Itof(v));
    std::fprintf(f, "%d\n", Ftoi(v));
    std::fprintf(f, "%d\n", Ftoi(Itof(v)));
  }
}

void DumpVec(std::FILE* f) {
  for (auto c : kVecCases) {
    IVec2 a(c.ax, c.ay), b(c.bx, c.by);
    IVec2 add = a + b, sub = a - b, mul = a * c.s, dv = a / c.s;
    std::fprintf(f, "%d\n%d\n", add.x, add.y);
    std::fprintf(f, "%d\n%d\n", sub.x, sub.y);
    std::fprintf(f, "%d\n%d\n", mul.x, mul.y);
    std::fprintf(f, "%d\n%d\n", dv.x, dv.y);
  }
}

void DumpSqrt(std::FILE* f) {
  for (auto c : kSqrtCases) {
    std::fprintf(f, "%d\n", VectorLength(c.x, c.y));
  }
}

void DumpCossin(std::FILE* f) {
  for (int i = 0; i < 128; ++i) {
    std::fprintf(f, "%d %d\n", cossin_table[i].x, cossin_table[i].y);
  }
}

void DumpRng(std::FILE* f) {
  Rand r;  // seed 0x1337 per rand.hpp
  for (int i = 0; i < 10000; ++i) {
    std::fprintf(f, "%u\n", r());
  }
  uint32_t const kMaxes[] = {1, 2, 7, 100, 128, 65536};
  for (uint32_t m : kMaxes) {
    for (int i = 0; i < 100; ++i) {
      std::fprintf(f, "%u\n", r(m));
    }
  }
  r.Seed(42);
  for (int i = 0; i < 100; ++i) {
    std::fprintf(f, "%u\n", r());
  }
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: oracle_dump <output-dir>\n");
    return 1;
  }
  PrecomputeTables();
  std::string const dir = argv[1];
  struct Entry {
    char const* name;
    void (*fn)(std::FILE*);
  } const entries[] = {{"fixed.txt", DumpFixed}, {"vec.txt", DumpVec},
                       {"sqrt.txt", DumpSqrt},   {"cossin.txt", DumpCossin},
                       {"rng.txt", DumpRng}};
  for (auto e : entries) {
    std::string const path = dir + "/" + e.name;
    std::FILE* f = std::fopen(path.c_str(), "w");
    if (!f) {
      std::fprintf(stderr, "cannot open %s\n", path.c_str());
      return 1;
    }
    e.fn(f);
    std::fclose(f);
  }
  return 0;
}
