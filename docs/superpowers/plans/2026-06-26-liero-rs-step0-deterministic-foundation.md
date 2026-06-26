# Steg 0 — Deterministisk grund: Implementationsplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Skapa Rust-craten `sim-core` (fixpunkt, vektor, heltals-sqrt, cossin-tabell, MT19937-RNG) och bevisa bit-exakt likhet med C++-motorn via golden-vektorer genererade ur C++.

**Architecture:** Ren Rust-crate utan Bevy-beroende. En liten C++-dumpare (`oracle_dump`) kör de *befintliga* C++-funktionerna och skriver deterministiska resultat till textfiler. Rust-tester läser samma filer och kräver identisk output rad för rad.

**Tech Stack:** Rust (stable, edition 2021), cargo workspace; C++20 (clang++) för dumparen.

## Global Constraints

- `sim-core` har **inga** beroenden (ingen `rand`-crate; MT19937 skrivs för hand).
- All aritmetik som kan svämma över använder `wrapping_*` för att matcha C++ 2-komplement-wrap.
- Heltalsskift (`<<`, `>>`) och heltalsdivision (`/`) ska användas **exakt** som i C++ — `>>` på `i32` är aritmetiskt (matchar C++ signed shift); `/` trunkerar mot 0 (matchar C++).
- `fixed` = `i32`, 16.16-format (`FRAC_BITS = 16`).
- Rust-koden bor i `rust/` i repo-roten; C++-dumparen i `src/tools/oracle_dump/`.
- Golden-filer committas i `rust/oracle-tests/golden/`.
- Källreferens (oraklet, ändras ej): `src/game/math.hpp`, `src/game/math.cpp`, `src/game/math/rect.hpp`, `src/game/rand.hpp`.

---

### Task 1: Cargo-workspace och tomma crates

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/sim-core/Cargo.toml`
- Create: `rust/sim-core/src/lib.rs`
- Create: `rust/oracle-tests/Cargo.toml`
- Create: `rust/oracle-tests/src/lib.rs`
- Create: `rust/.gitignore`

**Interfaces:**
- Consumes: inget.
- Produces: workspace där `sim_core` är en lib-crate och `oracle-tests` en crate med `dev-dependency` på `sim_core`.

- [ ] **Step 1: Skapa workspace-manifestet**

`rust/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["sim-core", "oracle-tests"]
```

- [ ] **Step 2: Skapa sim-core-manifest och lib**

`rust/sim-core/Cargo.toml`:
```toml
[package]
name = "sim-core"
version = "0.1.0"
edition = "2021"

[dependencies]
```

`rust/sim-core/src/lib.rs`:
```rust
//! Deterministisk simuleringskärna för Liero-rs. Ingen Bevy-, std-rng- eller
//! flyttalsberoende — allt är heltalsaritmetik som matchar C++-motorn bit-exakt.
```

- [ ] **Step 3: Skapa oracle-tests-crate**

`rust/oracle-tests/Cargo.toml`:
```toml
[package]
name = "oracle-tests"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
sim-core = { path = "../sim-core" }
```

`rust/oracle-tests/src/lib.rs`:
```rust
//! Golden-vektor-tester mot C++-oraklet. Se tests/.
```

`rust/.gitignore`:
```
/target
```

- [ ] **Step 4: Verifiera att workspace bygger**

Run: `cd rust && cargo build`
Expected: PASS (`Compiling sim-core`, `Compiling oracle-tests`, `Finished`).

- [ ] **Step 5: Commit**

```bash
git add rust/
git commit -m "feat(rust): scaffold sim-core + oracle-tests cargo workspace"
```

---

### Task 2: C++ golden-generator

**Files:**
- Create: `src/tools/oracle_dump/main.cpp`
- Create: `rust/oracle-tests/gen_golden.sh`
- Create (genererade, committas): `rust/oracle-tests/golden/fixed.txt`, `vec.txt`, `sqrt.txt`, `cossin.txt`, `rng.txt`

**Interfaces:**
- Consumes: `src/game/math.cpp`, `src/game/math.hpp`, `src/game/math/rect.hpp`, `src/game/rand.hpp`.
- Produces: fem golden-textfiler. **Format:** ett heltal per rad (decimal, i32/u32), utom `cossin.txt` som har `x y` per rad. Talsekvenserna definieras av de exakta indatalistorna nedan — Rust-testerna i Task 3–7 använder *identiska* listor.

- [ ] **Step 1: Skriv dumparen**

`src/tools/oracle_dump/main.cpp`:
```cpp
// Genererar golden-vektorer ur C++-oraklet för Rust-differentialtester.
// Kompileras fristående (se rust/oracle-tests/gen_golden.sh) — ingår ej i CMake.
#include <cstdint>
#include <cstdio>
#include <string>

#include "math.hpp"
#include "rand.hpp"

namespace {

// MÅSTE vara identiska med Rust-testernas listor.
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
  Rand r;  // seed 0x1337 enligt rand.hpp
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
```

- [ ] **Step 2: Skriv generator-skriptet**

`rust/oracle-tests/gen_golden.sh`:
```bash
#!/usr/bin/env bash
# Bygger C++-dumparen och genererar golden-vektorer. Kör från repo-roten.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/rust/oracle-tests/golden"
mkdir -p "$OUT"
BIN="$(mktemp -d)/oracle_dump"
clang++ -std=c++20 -O2 -I "$ROOT/src/game" \
  "$ROOT/src/game/math.cpp" \
  "$ROOT/src/tools/oracle_dump/main.cpp" \
  -o "$BIN"
"$BIN" "$OUT"
echo "golden written to $OUT"
```

- [ ] **Step 3: Generera golden-filerna**

Run: `chmod +x rust/oracle-tests/gen_golden.sh && ./rust/oracle-tests/gen_golden.sh`
Expected: `golden written to .../rust/oracle-tests/golden`

- [ ] **Step 4: Verifiera filinnehållet**

Run: `wc -l rust/oracle-tests/golden/*.txt`
Expected: `cossin.txt` = 128 rader; `fixed.txt` = 36 rader (12 inputs × 3); `rng.txt` = 10700 rader (10000 + 6×100 + 100); `vec.txt` = 40 rader (5 fall × 8); `sqrt.txt` = 7 rader.

- [ ] **Step 5: Commit**

```bash
git add src/tools/oracle_dump/main.cpp rust/oracle-tests/gen_golden.sh rust/oracle-tests/golden/
git commit -m "feat(oracle): C++ golden-vector dumper + generated vectors"
```

---

### Task 3: Fixpunkt (`fixed.rs`)

**Files:**
- Create: `rust/sim-core/src/fixed.rs`
- Modify: `rust/sim-core/src/lib.rs`
- Test: `rust/oracle-tests/tests/fixed_golden.rs`

**Interfaces:**
- Consumes: `golden/fixed.txt`.
- Produces: `sim_core::fixed::{Fixed, FRAC_BITS, itof, ftoi}` där `Fixed = i32`, `itof(i32) -> Fixed`, `ftoi(Fixed) -> i32`.

- [ ] **Step 1: Skriv golden-testet (failing)**

`rust/oracle-tests/tests/fixed_golden.rs`:
```rust
use sim_core::fixed::{ftoi, itof};

const FIXED_INPUTS: [i32; 12] = [
    -2000000, -65537, -65536, -100, -1, 0, 1, 100, 65535, 65536, 65537, 2000000,
];

#[test]
fn fixed_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/fixed.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    for v in FIXED_INPUTS {
        for expected in [itof(v), ftoi(v), ftoi(itof(v))] {
            let want: i32 = lines.next().unwrap().parse().unwrap();
            assert_eq!(expected, want, "mismatch for input {v}");
        }
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 2: Kör testet, verifiera kompileringsfel/fail**

Run: `cd rust && cargo test -p oracle-tests --test fixed_golden`
Expected: FAIL (`unresolved import sim_core::fixed`).

- [ ] **Step 3: Implementera fixed.rs**

`rust/sim-core/src/fixed.rs`:
```rust
//! 16.16 fixpunkt. Port av src/game/math.hpp (Itof/Ftoi).
pub type Fixed = i32;
pub const FRAC_BITS: u32 = 16;

/// Heltal → fixpunkt: v << 16. Wrap matchar C++ 2-komplement.
#[inline]
pub fn itof(v: i32) -> Fixed {
    v.wrapping_shl(FRAC_BITS)
}

/// Fixpunkt → heltal: v >> 16 (aritmetiskt skift, som C++ signed >>).
#[inline]
pub fn ftoi(v: Fixed) -> i32 {
    v >> FRAC_BITS
}
```

- [ ] **Step 4: Exponera modulen**

Lägg till i `rust/sim-core/src/lib.rs`:
```rust
pub mod fixed;
```

- [ ] **Step 5: Kör testet, verifiera pass**

Run: `cd rust && cargo test -p oracle-tests --test fixed_golden`
Expected: PASS (`test fixed_matches_cpp_oracle ... ok`).

- [ ] **Step 6: Commit**

```bash
git add rust/sim-core/src/fixed.rs rust/sim-core/src/lib.rs rust/oracle-tests/tests/fixed_golden.rs
git commit -m "feat(sim-core): fixed-point itof/ftoi matching C++ oracle"
```

---

### Task 4: Vektor (`vec.rs`)

**Files:**
- Create: `rust/sim-core/src/vec.rs`
- Modify: `rust/sim-core/src/lib.rs`
- Test: `rust/oracle-tests/tests/vec_golden.rs`

**Interfaces:**
- Consumes: `golden/vec.txt`.
- Produces: `sim_core::vec::Vec2 { x: i32, y: i32 }` med `add`, `sub`, `mul(i32)`, `div(i32)`, `neg`, `zero()`. Semantik exakt som `IVec2` i `rect.hpp` (komponentvis; `mul`/`div` mot skalär; wrap på +,-,*).

- [ ] **Step 1: Skriv golden-testet (failing)**

`rust/oracle-tests/tests/vec_golden.rs`:
```rust
use sim_core::vec::Vec2;

// (ax, ay, bx, by, s) — identiskt med kVecCases i dumparen.
const CASES: [(i32, i32, i32, i32, i32); 5] = [
    (0, 0, 0, 0, 1),
    (100, -50, 7, 9, 3),
    (-65536, 65536, 100, -100, 100),
    (123456, -789012, -3, 5, 7),
    (2000000, -2000000, 1, 1, 2),
];

#[test]
fn vec_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/vec.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    let mut next = || -> i32 { lines.next().unwrap().parse().unwrap() };
    for (ax, ay, bx, by, s) in CASES {
        let a = Vec2::new(ax, ay);
        let b = Vec2::new(bx, by);
        for v in [a.add(b), a.sub(b), a.mul(s), a.div(s)] {
            assert_eq!(v.x, next());
            assert_eq!(v.y, next());
        }
    }
}
```

- [ ] **Step 2: Kör testet, verifiera fail**

Run: `cd rust && cargo test -p oracle-tests --test vec_golden`
Expected: FAIL (`unresolved import sim_core::vec`).

- [ ] **Step 3: Implementera vec.rs**

`rust/sim-core/src/vec.rs`:
```rust
//! Heltalsvektor. Port av IVec2 i src/game/math/rect.hpp.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
    #[inline]
    pub fn add(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_add(r.x), self.y.wrapping_add(r.y))
    }
    #[inline]
    pub fn sub(self, r: Vec2) -> Vec2 {
        Vec2::new(self.x.wrapping_sub(r.x), self.y.wrapping_sub(r.y))
    }
    #[inline]
    pub fn mul(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_mul(s), self.y.wrapping_mul(s))
    }
    #[inline]
    pub fn div(self, s: i32) -> Vec2 {
        Vec2::new(self.x.wrapping_div(s), self.y.wrapping_div(s))
    }
    #[inline]
    pub fn neg(self) -> Vec2 {
        Vec2::new(self.x.wrapping_neg(), self.y.wrapping_neg())
    }
}
```

- [ ] **Step 4: Exponera modulen**

Lägg till i `rust/sim-core/src/lib.rs`:
```rust
pub mod vec;
```

- [ ] **Step 5: Kör testet, verifiera pass**

Run: `cd rust && cargo test -p oracle-tests --test vec_golden`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/sim-core/src/vec.rs rust/sim-core/src/lib.rs rust/oracle-tests/tests/vec_golden.rs
git commit -m "feat(sim-core): Vec2 matching C++ IVec2 oracle"
```

---

### Task 5: Heltals-sqrt och vektorlängd (`math.rs`)

**Files:**
- Create: `rust/sim-core/src/math.rs`
- Modify: `rust/sim-core/src/lib.rs`
- Test: `rust/oracle-tests/tests/sqrt_golden.rs`

**Interfaces:**
- Consumes: `golden/sqrt.txt`.
- Produces: `sim_core::math::{isqrt, vector_length}` där `isqrt(u32) -> u32` (port av `Sqr`), `vector_length(i32, i32) -> i32` (port av `VectorLength`).

- [ ] **Step 1: Skriv golden-testet (failing)**

`rust/oracle-tests/tests/sqrt_golden.rs`:
```rust
use sim_core::math::vector_length;

const CASES: [(i32, i32); 7] = [
    (0, 0), (3, 4), (100, 0), (0, 255), (1000, 1000), (-1234, 5678), (32767, 32767),
];

#[test]
fn vector_length_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/sqrt.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    for (x, y) in CASES {
        let want: i32 = lines.next().unwrap().parse().unwrap();
        assert_eq!(vector_length(x, y), want, "mismatch for ({x},{y})");
    }
}
```

- [ ] **Step 2: Kör testet, verifiera fail**

Run: `cd rust && cargo test -p oracle-tests --test sqrt_golden`
Expected: FAIL (`unresolved import sim_core::math`).

- [ ] **Step 3: Implementera math.rs**

`rust/sim-core/src/math.rs`:
```rust
//! Heltals-kvadratrot och vektorlängd. Port av Sqr/VectorLength i src/game/math.cpp.

/// Heltals-sqrt (avrundat nedåt). Bit-för-bit-port av Sqr().
pub fn isqrt(mut op: u32) -> u32 {
    let mut res: u32 = 0;
    let mut one: u32 = 1 << 30; // högsta fyrpotens
    while one > op {
        one >>= 2;
    }
    while one != 0 {
        if op >= res + one {
            op -= res + one;
            res += 2 * one;
        }
        res >>= 1;
        one >>= 2;
    }
    res
}

/// Port av VectorLength: isqrt(x*x + y*y), i32-aritmetik som castas till u32.
pub fn vector_length(x: i32, y: i32) -> i32 {
    let sum = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
    isqrt(sum as u32) as i32
}
```

- [ ] **Step 4: Exponera modulen**

Lägg till i `rust/sim-core/src/lib.rs`:
```rust
pub mod math;
```

- [ ] **Step 5: Kör testet, verifiera pass**

Run: `cd rust && cargo test -p oracle-tests --test sqrt_golden`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/sim-core/src/math.rs rust/sim-core/src/lib.rs rust/oracle-tests/tests/sqrt_golden.rs
git commit -m "feat(sim-core): integer isqrt + vector_length matching C++ oracle"
```

---

### Task 6: Cossin-tabell (`tables.rs`)

**Files:**
- Create: `rust/sim-core/src/tables.rs`
- Modify: `rust/sim-core/src/lib.rs`
- Test: `rust/oracle-tests/tests/cossin_golden.rs`

**Interfaces:**
- Consumes: `golden/cossin.txt`, `sim_core::vec::Vec2`.
- Produces: `sim_core::tables::precompute_cossin() -> [Vec2; 128]` (port av `PrecomputeTables`).

- [ ] **Step 1: Skriv golden-testet (failing)**

`rust/oracle-tests/tests/cossin_golden.rs`:
```rust
use sim_core::tables::precompute_cossin;

#[test]
fn cossin_table_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/cossin.txt"
    ))
    .unwrap();
    let table = precompute_cossin();
    let mut lines = golden.lines();
    for (i, entry) in table.iter().enumerate() {
        let line = lines.next().unwrap();
        let mut it = line.split_whitespace();
        let wx: i32 = it.next().unwrap().parse().unwrap();
        let wy: i32 = it.next().unwrap().parse().unwrap();
        assert_eq!(entry.x, wx, "x mismatch at index {i}");
        assert_eq!(entry.y, wy, "y mismatch at index {i}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 2: Kör testet, verifiera fail**

Run: `cd rust && cargo test -p oracle-tests --test cossin_golden`
Expected: FAIL (`unresolved import sim_core::tables`).

- [ ] **Step 3: Implementera tables.rs**

`rust/sim-core/src/tables.rs`:
```rust
//! cossin_table[128], port av PrecomputeTables i src/game/math.cpp.
//! Egen heltals-Taylorserie (i64) — ingen libm, fullt reproducerbar.
use crate::vec::Vec2;

struct Fp {
    s: i64,
    bits: i32,
}

impl Fp {
    fn reduce(&mut self, tobits: i32) {
        let lim: i64 = 1i64 << tobits;
        while self.s < (-lim - 1) || self.s > lim {
            self.s >>= 1;
            self.bits -= 1;
        }
    }
    fn reducedfrac(&self, tobits: i32) -> i64 {
        let mut rs = self.s;
        let mut rbits = self.bits;
        while rbits > 60 {
            rs >>= 1;
            rbits -= 1;
        }
        rs << (tobits - rbits)
    }
}

pub fn precompute_cossin() -> [Vec2; 128] {
    const SCALE_BITS: i32 = 28;
    const SCALE: i32 = 13176795; // (2pi / 128) << scalebits
    let mut table = [Vec2::zero(); 128];
    for i in 0..128i32 {
        let mut rf: i64 = 0;
        let mut c: i32 = -1;
        let xf: i32 = i * SCALE;
        let mut num = Fp {
            s: xf as i64,
            bits: SCALE_BITS,
        };
        let mut t: i32 = 1;
        while t < 26 {
            rf += (c as i64) * num.reducedfrac(60);

            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            t += 1;
            num.s /= t as i64;
            num.reduce(31);
            num.s = num.s.wrapping_mul(xf as i64);
            num.bits += SCALE_BITS;

            c = -c;
        }
        const SHIFT: i32 = 60 - 16;
        rf += 1i64 << (SHIFT - 1); // korrekt avrundning
        let r = (rf >> SHIFT) as i32;
        table[i as usize].x = r;
        table[((i + 32) & 0x7f) as usize].y = r;
    }
    table
}
```

- [ ] **Step 4: Exponera modulen**

Lägg till i `rust/sim-core/src/lib.rs`:
```rust
pub mod tables;
```

- [ ] **Step 5: Kör testet, verifiera pass**

Run: `cd rust && cargo test -p oracle-tests --test cossin_golden`
Expected: PASS (alla 128 poster matchar — bevisar Taylor-porten).

- [ ] **Step 6: Commit**

```bash
git add rust/sim-core/src/tables.rs rust/sim-core/src/lib.rs rust/oracle-tests/tests/cossin_golden.rs
git commit -m "feat(sim-core): cossin table via integer Taylor series matching C++ oracle"
```

---

### Task 7: RNG (`rng.rs`)

**Files:**
- Create: `rust/sim-core/src/rng.rs`
- Modify: `rust/sim-core/src/lib.rs`
- Test: `rust/oracle-tests/tests/rng_golden.rs`

**Interfaces:**
- Consumes: `golden/rng.txt`.
- Produces: `sim_core::rng::Rand` med `new()` (seed `0x1337`), `seed(u32)`, `next_u32() -> u32`, `bound(u32) -> u32` (Lemire), `bound_range(u32, u32) -> u32`.

- [ ] **Step 1: Skriv golden-testet (failing)**

`rust/oracle-tests/tests/rng_golden.rs`:
```rust
use sim_core::rng::Rand;

#[test]
fn rng_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/golden/rng.txt"
    ))
    .unwrap();
    let mut lines = golden.lines();
    let mut next_golden = || -> u32 { lines.next().unwrap().parse().unwrap() };

    let mut r = Rand::new(); // seed 0x1337
    for i in 0..10000 {
        assert_eq!(r.next_u32(), next_golden(), "raw mismatch at {i}");
    }
    for m in [1u32, 2, 7, 100, 128, 65536] {
        for i in 0..100 {
            assert_eq!(r.bound(m), next_golden(), "bound({m}) mismatch at {i}");
        }
    }
    r.seed(42);
    for i in 0..100 {
        assert_eq!(r.next_u32(), next_golden(), "reseed mismatch at {i}");
    }
    assert!(lines.next().is_none(), "extra golden lines");
}
```

- [ ] **Step 2: Kör testet, verifiera fail**

Run: `cd rust && cargo test -p oracle-tests --test rng_golden`
Expected: FAIL (`unresolved import sim_core::rng`).

- [ ] **Step 3: Implementera rng.rs**

`rust/sim-core/src/rng.rs`:
```rust
//! Deterministisk MT19937, port av std::mt19937-användningen i src/game/rand.hpp.
//! Standard-parametrar; single-seed-init matchar C++:s std::mt19937(seed).

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

pub struct Rand {
    mt: [u32; N],
    idx: usize,
    #[allow(dead_code)]
    last: u32,
}

impl Rand {
    pub fn new() -> Self {
        let mut r = Rand {
            mt: [0; N],
            idx: N + 1,
            last: 0,
        };
        r.seed(0x1337);
        r
    }

    pub fn seed(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.idx = N;
        self.last = 0;
    }

    fn generate(&mut self) {
        for i in 0..N {
            let y = (self.mt[i] & UPPER_MASK) | (self.mt[(i + 1) % N] & LOWER_MASK);
            let mut next = self.mt[(i + M) % N] ^ (y >> 1);
            if y & 1 != 0 {
                next ^= MATRIX_A;
            }
            self.mt[i] = next;
        }
        self.idx = 0;
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.idx >= N {
            self.generate();
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        self.last = y;
        y
    }

    /// [0, max) via Lemire multiply-shift (matchar rand.hpp).
    pub fn bound(&mut self, max: u32) -> u32 {
        ((self.next_u32() as u64 * max as u64) >> 32) as u32
    }

    /// [min, max).
    pub fn bound_range(&mut self, min: u32, max: u32) -> u32 {
        self.bound(max - min) + min
    }
}

impl Default for Rand {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Exponera modulen**

Lägg till i `rust/sim-core/src/lib.rs`:
```rust
pub mod rng;
```

- [ ] **Step 5: Kör testet, verifiera pass**

Run: `cd rust && cargo test -p oracle-tests --test rng_golden`
Expected: PASS (10 000 råa + bounded + reseed matchar — bevisar MT19937-porten).

- [ ] **Step 6: Commit**

```bash
git add rust/sim-core/src/rng.rs rust/sim-core/src/lib.rs rust/oracle-tests/tests/rng_golden.rs
git commit -m "feat(sim-core): MT19937 RNG matching C++ std::mt19937 oracle"
```

---

### Task 8: README och full verifiering

**Files:**
- Create: `rust/README.md`

**Interfaces:**
- Consumes: alla tidigare tasks.
- Produces: dokumentation av orakel-flödet + grön helhet.

- [ ] **Step 1: Skriv rust/README.md**

`rust/README.md`:
```markdown
# Liero-rs (Rust-omskrivning)

Cargo-workspace för den agent-drivna Rust/Bevy-omskrivningen av OpenLiero.
Se `../docs/superpowers/specs/2026-06-26-liero-rs-roadmap.md`.

## Crates

- `sim-core` — deterministisk kärna (fixpunkt, vektor, sqrt, cossin, RNG). Inga
  beroenden, ingen Bevy. Allt är heltalsaritmetik som matchar C++-motorn bit-exakt.
- `oracle-tests` — differentialtester mot C++-oraklet via golden-vektorer.

## Orakel-flödet

1. C++-dumparen (`src/tools/oracle_dump`) kör de *befintliga* C++-funktionerna och
   skriver deterministiska resultat till `oracle-tests/golden/*.txt`.
2. Rust-testerna läser samma filer och kräver identisk output rad för rad.

Regenerera golden efter ändring i C++-`math`/`rand`:

\`\`\`bash
./oracle-tests/gen_golden.sh
\`\`\`

## Köra testerna

\`\`\`bash
cd rust && cargo test
\`\`\`
```

- [ ] **Step 2: Kör hela sviten**

Run: `cd rust && cargo test`
Expected: PASS — fem golden-tester gröna (fixed, vec, sqrt, cossin, rng).

- [ ] **Step 3: Verifiera determinism vid regenerering (idempotens)**

Run: `./rust/oracle-tests/gen_golden.sh && cd rust && git diff --stat -- oracle-tests/golden && cargo test`
Expected: `git diff` tomt (golden oförändrade), `cargo test` PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/README.md
git commit -m "docs(rust): document oracle differential-testing workflow"
```

---

## Self-Review

**Spec coverage:** Varje punkt i steg 0-specens "Scope (Ingår)" har en task: workspace (T1), fixpunkt (T3), vektor (T4), VectorLength/Sqr (T5), cossin (T6), Rand inkl. Lemire (T7), C++ golden-generator (T2), Rust golden-tester (T3–T7). "Definition of done" täcks: cossin 128 (T6), RNG ≥10000 (T7), fixpunkt inkl. negativa/gränsfall (T3), generator reproducerbar (T2/T8 idempotenskoll), README (T8).

**Placeholder scan:** Inga TBD/TODO; all kod är fullständig och konkret.

**Type consistency:** `Fixed=i32`/`itof`/`ftoi` (T3), `Vec2{x,y}` med `new/zero/add/sub/mul/div/neg` (T4, använd i T6), `isqrt`/`vector_length` (T5), `precompute_cossin()->[Vec2;128]` (T6), `Rand::{new,seed,next_u32,bound,bound_range}` (T7) — namn och signaturer konsekventa mellan dumpare, tester och moduler. Golden-radantal i T2 stämmer med testernas läsordning i T3–T7.
```
