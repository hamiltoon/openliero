# Liero-rs — roadmap for an agent-driven Rust/Bevy rewrite

Status: **draft for review** · 2026-06-26

A strategy for rewriting the OpenLiero simulation in Rust + Bevy, incrementally,
with AI agents, in the **same repo** (monorepo) as the C++ engine. This document is
the roadmap altitude ("everything"); each step gets its own detailed spec just-in-time.

## Goals and motivations

- **Learn** modern Rust/Bevy/ECS and agent-driven development (the main point).
- **Reach**: web (Wasm) and eventually mobile.
- **New capabilities**: room for larger levels, more players, moddability.
- **Maintainability**: clear modules, heavily tested.
- Time is *no* constraint. It's allowed to be hard and take time.

## Guiding principle: determinism is the crown jewel

Liero's soul is **deterministic fixed-point simulation + rollback netcode**. 90% of
a rewrite is easy (sprites, menus, sound). The last 10% — making tick #1000
come out *bit-exact* on two machines — is the hard part, and what makes replay and
rollback netplay possible. A rewrite that loses determinism loses the game.

Therefore: **no big-bang rewrite.** Strangler pattern — build the new engine
piece by piece, prove each piece correct against the old one, keep the old one until the
new one is done.

## Modernization charter

This is a **modernization, not a literal port.** The line:

- **Locked, bit-exact (what we keep):** the deterministic simulation — all of
  `processFrame`'s math, fixed-point, RNG, lookup tables, and ordering — plus the
  parsed data that drives it (the material map, weapon/object parameters), and the
  ability to read existing `data/` assets. Replay, rollback, and the game's feel
  depend on tick #1000 being identical. These keep the differential-testing oracle.
- **Free to modernize (we do NOT mirror C++):** code architecture, module/crate
  boundaries, APIs, error handling, and idioms — idiomatic Rust (`Result`,
  `std::io`, iterators, `serde` where apt) rather than a transliteration of C++
  structures (e.g. use `std::io`/`from_le_bytes`, not a port of `io::Reader`).
  Internal data representations, naming, build, and test layout are ours to choose.

In short: the *behaviour* that affects the simulation is sacred and bit-exact;
*how the code is structured* we rebuild cleanly. The oracle proves we preserved
the behaviour while modernizing the implementation.

## The oracle strategy: differential testing against C++

The existing C++ engine is kept as the **oracle of truth**. For each subsystem in
the new engine, exactly the same input is fed through both, and the state is compared
by checksum, tick by tick. If they match bit-for-bit, the new part is *proven*
correct; otherwise the diff points out exactly which tick diverges.

This is what makes the agent work safe: a large, scary rewrite becomes a
long series of **small, independent, objectively verifiable tasks** — the form agents
are best at. The monorepo makes it easy: both codebases sit side by side and CI
can run "new engine matches old engine" as an ordinary test.

## Technology choices

| Layer | Choice | Why |
|---|---|---|
| Language | **Rust** | Integer math, no GC → natural determinism; Wasm target; the type system makes agent code safer |
| Engine | **Bevy** (ECS) | Modern, code-first, ECS fits entity-heavy games; batteries included |
| Rollback | **bevy_ggrs** (GGRS) | Mature rollback ecosystem; per-frame checksum reused as the oracle |
| RNG | **bevy_rand** + ported MT19937 | RNG must be part of the rollback state, seeded and restorable |
| Determinism core | dedicated `sim-core` crate, **no Bevy dependency** | Protects fixed-point/RNG from Bevy's API churn; tested in isolation |

**The Bevy trap to handle:** Bevy runs systems in parallel and in
non-guaranteed order. Rollback requires the opposite. bevy_ggrs solves this via a `GgrsSchedule`
(fixed rate, locked system order), explicit registration of rollback state, and
RNG as part of that state. Determinism discipline in the sim systems is a requirement,
not a bonus.

## Monorepo layout

The Rust code lives in its own top-level directory, parallel to `src/` (C++) and
`server/` (Go):

```
openliero/
├── src/            C++ engine (the oracle, kept)
├── server/         Go signaling/relay (kept)
├── rust/           ← NEW: cargo workspace
│   ├── sim-core/     pure Rust, no Bevy: fixed, rng, vec, tables
│   ├── game/         the Bevy app (ECS components, systems, rendering)
│   └── oracle-tests/ differential tests against C++ (golden vectors)
└── data/           shared assets (same data = same oracle)
```

## The steps (strangler order)

Each step is differential-tested against C++ before the next begins.

| # | Step | Done when … |
|---|---|---|
| **0** | **Deterministic foundation** — `sim-core`: fixed-point + RNG | Rust reproduces C++'s `math` and `Rand` bit-exact (golden vectors green) |
| 1 | **Asset/data formats** — read levels, TC, sprites | Rust loads the same `data/` files as C++ and gets identical bytes |
| 2 | **Sim core in ECS** — Level → Worm → one weapon → `processFrame` | A bullet is fired, moves, explodes, destroys terrain — checksum matches C++ tick by tick |
| 3 | **Rendering** — Bevy draws the world | Playable image in a window and in the browser (Wasm) |
| 4 | **Loop + input** — fixed rate, keyboard | Playable single-player, feels like Liero |
| 5 | **bevy_ggrs** — rollback netplay | Two clients play the same match, desync-free |

Steps 2–5 are detailed-specced just-in-time; we understand them better after steps 0–1.

## Risks and how the oracle de-risks them

- **Subtle determinism divergence** → caught immediately by the checksum diff; we get
  exactly which tick and which field differs.
- **Bevy API churn** → determinism-critical parts are isolated in `sim-core` with no
  Bevy dependency.
- **Agent produces plausible but incorrect code** → cannot pass the oracle;
  correctness is objective, not a matter of judgment.
- **Scope creep** → strangler + just-in-time spec prevent over-planning parts we
  don't yet understand.

## Deliberately deferred (YAGNI for now)

Mobile packaging, new game modes, larger levels, mod tools, 3D. All of this becomes
possible *after* a deterministic core exists — but none of it may
complicate steps 0–2.

## Next concrete artifact

Detailed spec for **step 0**:
`2026-06-26-liero-rs-step0-deterministic-foundation-design.md`.
