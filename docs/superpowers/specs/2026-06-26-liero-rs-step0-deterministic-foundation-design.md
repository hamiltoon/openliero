# Steg 0 — Deterministisk grund (`sim-core`)

Status: **utkast för granskning** · 2026-06-26
Del av: `2026-06-26-liero-rs-roadmap.md`

## Syfte

Bevisa den enda riktigt riskabla hypotesen i hela projektet **innan** vi bygger
något ovanpå den: *kan Rust reproducera C++-motorns fixpunktsmatematik och RNG
bit-exakt?* Om ja → vi har orakel-mönstret som alla senare steg lutar sig mot. Om
nej → vi vet det på en dag, inte efter månader.

Steg 0 levererar `sim-core`-craten (fixpunkt + RNG + tabeller) plus en
**differentialtest** som kör samma operationer genom C++ och Rust och kräver
identisk output.

## Varför det går att lyckas (förundersökning klar)

Källan är redan granskad och är **ren heltalsaritmetik** — inga flyttal, ingen
libm, inga plattformsberoenden:

- **`math.hpp` / `math.cpp`** — `fixed = int32` i 16.16-format. `Itof = v<<16`,
  `Ftoi = v>>16` (aritmetiskt skift). `cossin_table[128]` byggs med en egen
  **heltals-Taylorserie** (int64), inte `sin/cos`. `VectorLength` använder en
  **heltals-kvadratrot** (`Sqr`). Allt reproducerbart.
- **`rand.hpp`** — `std::mt19937` (standard MT19937, 32-bit) seedad med `0x1337`.
  Bounded form använder **Lemire**: `(u64(x) * max) >> 32`. MT19937 är en helt
  specificerad standardalgoritm → identisk i Rust.

## Scope

**Ingår:**
1. Cargo-workspace under `rust/` med craten `sim-core` (ingen Bevy-dep) och
   `oracle-tests`.
2. Port av fixpunkt: `Fixed` (i32, 16.16), `IVec2/fixedvec` med exakt samma
   operator-semantik som C++ (trunkering!).
3. Port av `VectorLength` / heltals-`Sqr`.
4. Port av `cossin_table`-genereringen (Taylor-serien) — och/eller verifiering mot
   golden.
5. Port av `Rand`: MT19937 + `next()`, `bound(max)` (Lemire), `bound(min,max)`,
   `seed()`.
6. C++ **golden-generator** (litet verktyg/test) som dumpar deterministiska
   vektorer till en committad fil.
7. Rust-test som läser golden-filen och kräver bit-identisk output.

**Ingår INTE** (senare steg): Bevy, ECS, rendering, asset-laddning, någon
spel-logik, RNG-state-serialisering i C++:s textformat (vi jämför *output*, inte
state-format).

## Arkitektur

```
rust/
├── Cargo.toml                # [workspace] members = ["sim-core", "oracle-tests"]
├── sim-core/
│   ├── Cargo.toml            # inga beroenden (ev. bara för MT19937 om vi ej skriver egen)
│   └── src/
│       ├── lib.rs
│       ├── fixed.rs          # Fixed(i32, 16.16): itof, ftoi, mul/div-semantik
│       ├── vec.rs            # IVec2/fixedvec + operatorer
│       ├── tables.rs         # cossin_table[128], precompute (Taylor)
│       └── rng.rs            # Rand: MT19937 + Lemire-bound
└── oracle-tests/
    ├── Cargo.toml
    ├── tests/
    │   ├── fixed_golden.rs
    │   ├── tables_golden.rs
    │   └── rng_golden.rs
    └── golden/               # committade vektorer genererade av C++
        ├── fixed.txt
        ├── cossin.txt
        └── rng.txt

src/tools/oracle_dump/        # NYTT C++-verktyg som genererar golden/*.txt
```

`sim-core` har medvetet **inget Bevy-beroende** — determinismen isoleras från
engine-churn och kan testas helt fristående.

## Semantik som MÅSTE bevaras (bit-exakthetens detaljer)

| C++ | Rust | Fälla |
|---|---|---|
| `fixed = int` (32-bit) | `i32` | Anta 32-bit `int`; använd `i32`, inte `i64` |
| `Itof(v) = v << 16` | `v << 16` | Overflow vid stora `v` ska matcha (wrap) |
| `Ftoi(v) = v >> 16` | `v >> 16` | Aritmetiskt skift på negativa (rundar mot −∞) — Rust `>>` på `i32` gör samma |
| `a * b / 100` (IVec2) | `a * b / 100` | Heltalsdivision trunkerar mot 0 i båda — men skift och division rundar OLIKA för negativa; bevara exakt operator |
| `Sqr` heltals-sqrt | port rakt av | u32-aritmetik, exakt loop |
| cossin Taylor (int64) | i64 | `Reduce`/`Reducedfrac`-skiften måste matcha exakt |
| MT19937 seed `0x1337` | samma init | C++:s single-seed init = referens-`init_genrand`; en trogen MT matchar |
| `bound = (u64(x)*max)>>32` | samma | Enkelt; inga distributionsskillnader |

Overflow: C++ signed overflow är formellt UB men i praktiken wrap på mål-plattform.
Rust `i32` overflow panikar i debug. → använd **`wrapping_*`** där C++ förlitar sig
på wrap, eller bekräfta via golden att inga overflows sker i testdatan. Beslut tas
när golden genereras (se öppna frågor).

## Differentialtest-harness

Enklast och mest CI-vänligt: **generera golden från C++ en gång, committa, testa
Rust mot dem.** Slipper länka C++ från Rust.

**C++ golden-generator** (`src/tools/oracle_dump/`) — en liten `main()` som:
- `fixed.txt`: för ett bestämt rutnät av `v` (inkl. negativa, gränsvärden) skriver
  `Itof/Ftoi`-rundturer; för par `(a,b)` skriver `IVec2`-add/sub/mul/div-resultat.
- `cossin.txt`: alla 128 `cossin_table`-poster `(x, y)` efter `PrecomputeTables()`.
- `rng.txt`: seed `0x1337` → första N råa `engine()`-värden; bounded `bound(max)`
  för flera `max`; en reseed-sekvens.

**Rust-tester** läser respektive golden-fil och `assert_eq!` rad för rad. En enda
divergens = rött, med exakt index.

Detta gör steg 0 till en perfekt **agent-uppgift**: målet är binärt och objektivt.

## Definition of done

- [ ] `cargo test` i `rust/` är grönt: alla tre golden-sviter matchar.
- [ ] `cossin.txt` (128 poster) matchar bit-för-bit — bevisar Taylor-porten.
- [ ] RNG: ≥ 10 000 råa MT-värden + bounded-värden matchar.
- [ ] Fixpunkt: rundtur + aritmetik inkl. negativa/gränsfall matchar.
- [ ] Golden-generatorn är committad och reproducerbar (`make`/CMake-target).
- [ ] Kort README i `rust/` som förklarar orakel-flödet.

## Öppna frågor (att besluta innan/under bygget)

1. **MT19937 i Rust:** skriva egen (~40 rader, full kontroll, ingen dep) eller
   använda en crate (`rand_mt`/`mt19937`)? Måste matcha C++:s single-seed init.
   *Förslag:* skriv egen — liten, ingen dep, exakt kontroll.
2. **Overflow-policy:** `wrapping_*` överallt, eller bevisa via golden att testdatan
   aldrig svämmar? *Förslag:* `wrapping_*` i `Fixed`/`IVec2` för att matcha C++ på
   alla indata.
3. **Golden-format:** ren text (en post per rad) räcker och är diff-vänligt.
4. **CMake-integration nu eller senare?** Golden-generatorn kan vara ett enkelt
   CMake-target bakom en option; full cargo-i-CMake skjuts till steg 1+.

## Nästa steg efter detta spec

Granska specet → `writing-plans`-skill gör en konkret, stegvis implementationsplan
för steg 0 → bygg (gärna agent-drivet, en uppgift per golden-svit).
