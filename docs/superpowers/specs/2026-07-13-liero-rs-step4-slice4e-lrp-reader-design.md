# Step 4, Slice 4e — `.lrp` byte-faithful reader (the C++-interop gate): Design

Status: **DESIGN — Slice 4e** · 2026-07-13 · slices 4a–4d LANDED, 4e next (largest Step-4 surface)
Part of: `2026-06-26-liero-rs-roadmap.md`
Companion overview: `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited **overview §N**; the 4e
bullet + Open Q1/Q2 resolutions + Locked decision 4)
Built on: `2026-07-12-liero-rs-step4-cpp-input-replay-map.md` (cited **input-map §N**; §3 container,
§5 `WideRollbackChecksum`)
Precedent: the 4b record/replay design + the 4d live-viewport design (dumper/re-diff discipline)

This slice reads a real Liero `.lrp` replay *byte-faithfully* and gates it against C++ ground truth.
Per **overview §Open Q1** the slice is **split into two phases**; this design pins exactly where the
phase boundary falls, why, and how each phase is gated. **Phase 1 is this slice.** Phase 2 is a
bounded follow-on that may become its own slice or a post-Step-4 item.

---

## 0. The two-phase split (the adjudicated scope)

**Phase 1 (slice 4e, this plan): container + delta-stream + `WideRollbackChecksum`, gated against a
Rust-reconstructed initial state over a C++-generated corpus.** No cereal parsing.

**Phase 2 (bounded follow-on): cereal `Game` deserialization → full `framehash` render-diff over
arbitrary real `.lrp` files.** The single largest unknown in Step 4 (input-map §3a, §8.5).

The enabler that makes this a *clean* cut rather than an arbitrary one: **every cereal blob in the
`.lrp` is length-prefixed.** `CerealWrite` writes `[uint32 length][blob]` (`replay.cpp:60-71`), and
the mid-stream `Settings`/`WormSettings` tags use the same helper (`replay.cpp:333,351`). So a Phase-1
reader can **skip the entire cereal `Game` graph and every mid-stream settings blob** by reading the
`uint32` length and advancing that many bytes — it never has to understand cereal at all. The cereal
surface is not "partially parsed"; in Phase 1 it is *skipped whole*.

Why Phase 1 can gate meaningfully without parsing the initial state:

- The `.lrp`'s initial `Game` is reconstructable **from the outside**. If the corpus generator
  (C++) builds its initial `Game` from the *same* scenario file the Rust side already loads
  (`scenario::loader::load`, `rust/scenario/src/loader.rs:100`), then Rust can recreate the identical
  tick-0 `SimState` **without** touching the cereal blob — same seed/level/worms/weapons.
- `WideRollbackChecksum` (input-map §5, `replay.cpp:177-258`) folds the **entire** rollback-state
  inventory (RNG, cycles, every worm field, all four projectile/bonus pools, the whole material
  buffer). If Rust's scenario-driven sim tracks C++'s `ProcessFrame` bit-for-bit — which Steps 2/3
  already prove via `HashGameState` goldens — then the Rust-computed `WideRollbackChecksum` equals
  the word the `.lrp` embedded every 1050 frames. **That equality is a strictly *harder* gate than
  `HashGameState`** (it folds more fields), so passing it proves the stream semantics + the full
  state inventory, which is exactly what "byte-faithful reading of the container/delta stream" means.

Recommendation, restated concretely: **land Phase 1 as slice 4e; defer Phase 2** and only promote it
to a slice once the container/stream reader is green. If the cereal `Game` graph proves larger than
the rest of Step 4 combined (likely — it is the whole `serialization/cereal_types.hpp` graph), it
becomes its own post-Step-4 item. The container/stream reader ships in Step 4 regardless (honoring
Locked decision 4's "byte-faithful reading" for the parts that do not require cereal).

### Where the phase boundary falls, practically

| Concern | Phase 1 (4e) | Phase 2 (follow-on) |
|---|---|---|
| `LRPF` magic + version byte (`replay.cpp:112-123`) | **read + validate** | — |
| whole-stream deflate inflate (`replay.cpp:99-110`) | **read** (inflate-to-memory) | — |
| initial `Game` cereal blob (`replay.cpp:129`) | **skip** via `uint32` length prefix | **parse** |
| mid-stream `Settings`/`WormSettings` tags 0x81/0x82 (`replay.cpp:271-289`) | **skip** via length prefix | apply |
| empty-frame 0x80 / end 0x83 (`replay.cpp:268,290`) | **decode** | decode |
| per-worm XOR-delta `<0x80` (`replay.cpp:293-307`) | **decode** | decode |
| `WideRollbackChecksum` word every 1050 (`replay.cpp:317-323`) | **consume + verify** | verify |
| version<7 legacy palette / worm-rgb expansion (`replay.cpp:132-143`) | N/A (palettes are render-only; not folded by the checksum) | apply |
| initial state source | **scenario-reconstructed** (`loader::load`) | **cereal-reconstructed** |
| gate | Rust `WideRollbackChecksum` == embedded words + `HashGameState` == `_sim.txt` golden | per-frame render `framehash` diff vs C++ |

The two version-legacy render expansions (`replay.cpp:132-143` palette `<<2`, `ExpandLegacyWormRgb`)
touch only the palette / worm colour — **not** any field `WideRollbackChecksum` folds — so they are
Phase-2 concerns (they matter only once we render). Phase 1 still reads the version byte, rejects
`version > kMyReplayVersion` (`replay.cpp:121`), and skips the length-prefixed settings tags
regardless of version.

---

## 1. `.lrp` binary layout (Phase-1-relevant facts, `file:line`)

Container, in order on the *inflated* byte stream (`replay.cpp`):

1. `uint32 magic == ('L'<<24)|('R'<<16)|('P'<<8)|'F'` — big-endian `LRPF` (`replay.cpp:112,116-119`).
2. `uint8 version` — `kMyReplayVersion`; reject `> kMyReplayVersion` (`replay.cpp:120-123`).
3. **initial `Game`**: `[uint32 len][cereal blob]` (`replay.cpp:129` via `CerealWrite`
   `replay.cpp:60-71`). **Phase 1 skips `len` bytes.**
4. Per-frame stream, repeated (`replay.cpp:260-326` reader / `:328-373` writer):
   - `0x80` **empty frame** — no input change; all worms keep last input (`replay.cpp:268`).
   - `0x81` **settings** — `[uint32 len][cereal Settings]` (`replay.cpp:271-279`). **Skip.**
   - `0x82` **worm settings** — `uint32 worm_idx` + `[uint32 len][cereal WormSettings]`
     (`replay.cpp:280-289`). **Skip both the idx and the blob.**
   - `0x83` **end** — end of stream (`replay.cpp:290-292`).
   - `< 0x80` **input frame** — first byte is worm-0's delta; then **one byte per remaining worm**
     in `game.worms` order (`replay.cpp:293-307`). Per worm:
     `control_states = Unpack(byte ^ prev_control_states.Pack())` (`replay.cpp:304`). Worm sequencing
     is **implicit in the loop** — there is no per-worm index in an input frame.
   - **After** an input/empty frame, iff `game.cycles % (70*15) == 0` (**every 1050 frames**), a
     `uint32 WideRollbackChecksum` word follows and is compared (`replay.cpp:317-323`). ⚠ The word is
     read at the *tail* of `PlaybackFrame`, i.e. against the state as it stood entering that
     playback step (the checksum is written in `RecordFrame` *before* the tick's `ProcessFrame`,
     `replay.cpp:369-372`, and `RecordFrame` runs before `ProcessFrame` in the loop —
     input-map §1b/§4). The reader must fold the checksum at the same point in its own loop.

Delta encoding (⚠ load-bearing, input-map §3c): the XOR baseline is `prev_control_states`, which C++
updates at `ProcessFrame`'s tail (`game.cpp:466-468`). A Phase-1 reader that decodes inputs must
maintain its own per-worm `prev_control_states = control_states` **after** applying each frame, so the
next frame's XOR baseline is correct. Rust `WormState` does **not** store `prev_control_states` (the
Rust sim replays absolute per-tick words — 4b done-report); the reader carries the baseline itself.

Deflate: the whole file is one deflate stream produced by **miniz** `mz_deflateInit(...,
MZ_DEFAULT_COMPRESSION)` (`src/game/io/deflate.hpp:3-4,136`), which emits a **zlib-wrapped** stream
(header `0x78 …` + trailing adler32). C++ inflates the *entire* file into a `std::vector<uint8_t>`
up front (`replay.cpp:99-110`) so `R` can rewind; the Phase-1 reader mirrors this (inflate-to-memory,
then parse from a byte cursor).

---

## 2. `WideRollbackChecksum` — the second hash to port (`replay.cpp:168-258`)

This is **not** `HashGameState`. It is a boost-style hash-combine (`Mix32`) over a *different, larger*
field set, in a *different* order. Port target: a new `sim::wide_checksum::wide_rollback_checksum(&SimState) -> u32`, sitting beside `hash_game_state` in `sim` (it folds `SimState`, so it belongs in
`sim`, parallel to `hash.rs`).

Mixer (`replay.cpp:168-174`), all `u32` wrapping:

```
Mix32(h, v):  h ^= v.wrapping_add(0x9e3779b9).wrapping_add(h << 6).wrapping_add(h >> 2)
MixBytes(h, p, n): for each byte b in p[..n]: Mix32(h, b as u32)
```

Fold order (exactly `replay.cpp:181-257`; every field cast `as u32` == C++ `static_cast<uint32_t>`):

1. `h = game.rand.last` (seed, `replay.cpp:181`); `Mix32(h, cycles)`.
2. **per worm** (`replay.cpp:184-212`), in `game.worms` order:
   `pos.x, pos.y, vel.x, vel.y, aiming_angle, aiming_speed, health, lives, kills, timer,
   killed_timer, current_frame, current_weapon, direction, flags, visible(0/1), ready(0/1),
   able_to_jump(0/1), able_to_dig(0/1), control_states.istate, prev_control_states.istate`, then
   **per weapon**: `ammo, delay_left, loading_left`.
3. `Mix32(h, wobjects.count)`, then **per wobject**: `pos.x, pos.y, vel.x, vel.y, cur_frame,
   time_left, owner_idx` (`replay.cpp:214-224`).
4. `Mix32(h, sobjects.count)`, then **per sobject**: `x, y, cur_frame, id` (`replay.cpp:225-232`).
5. `Mix32(h, nobjects.count)`, then **per nobject**: `pos.x, pos.y, vel.x, vel.y, cur_frame`
   (`replay.cpp:233-241`).
6. `Mix32(h, bonuses.count)`, then **per bonus**: `x, y, frame, timer` (`replay.cpp:242-249`).
7. `MixBytes(h, level.material_id, width*height)` (`replay.cpp:251-255`).

**State-inventory gaps vs the Rust `SimState` (verified against `rust/sim/src/state.rs`):**

- `worm.flags` (`worm.hpp:246` — *"how many flags does this worm have?"*, a **GameOfTag flag count**,
  not a bitfield). Rust `WormState` has **no** `flags` field. In the Phase-1 corpus (KillEmAll,
  `game_mode 0`, no flag pickups) it is always `0` → **fold a constant `0`**, documented. (If Phase 2
  ever covers a GameOfTag `.lrp`, add the field then.)
- `worm.prev_control_states` — Rust `WormState` has no such field. The **reader** maintains it (it
  must, to decode the next XOR delta); the gate harness folds the reader's baseline. Both
  `control_states.istate` and `prev_control_states.istate` are the masked 7-bit words (every value
  reaches the checksum via `Unpack`, `state & 0x7f`, `worm.hpp:159`), so `ControlState::pack()` on
  each side is the correct `istate`.
- All other worm fields exist: `aiming_speed, killed_timer, current_frame, current_weapon, direction,
  ready, able_to_jump, able_to_dig` (`state.rs:294-360`); pool fields `WObject.owner_idx/time_left`,
  `SObject.x/y`, `NObject.*`, `Bonus.frame/timer` all present (grep-confirmed).

A single mis-ordered or missing field silently desyncs **only at the 1050-frame cadence** (no earlier
signal), so the port needs a hand-folded unit test pinning the mixer + field order (mirroring the
`hash.rs` by-hand tests), and the corpus must exceed 1050 ticks so ≥1 embedded word is actually
consumed non-vacuously.

---

## 3. The corpus generator (a C++-side task; there are no `.lrp` in-repo)

`find -iname '*.lrp'` is empty (overview §Open Q2). We **generate** a small committed corpus from
C++ — the cheapest authentic oracle — reusing the existing scenario corpus so the input vectors
already have sim goldens (transitively validating 4b's native format against `.lrp` semantics).

**Construction:** a new headless tool (recommend `src/tools/oracle_dump/lrp_gen.cpp`, wired like the
other `oracle_dump_*` in `CMakeLists.txt:374-389`) that:

1. Parses the **same** scenario grammar as `sim_physics_dump.cpp` (reuse `ParseScenario` — the shared
   file must parse on both sides, exactly as 4d's `render_live` does).
2. Builds a **real `Game`** from the scenario **using the dumper's exact tick-0 setup** — same worm
   construction, same `resolve_weapons`, same blood-pool sizing that mirrors `StartGame`
   (`sim_physics_dump.cpp:396-460`), so tick-0 state is bit-identical to what Rust's
   `SimState::new` + `loader::load` produces (this alignment is load-bearing — see §5 risk 2).
3. Installs the **headless-safe** side-effect stubs — the traps the reduced dumper already documents:
   - `NullSoundPlayer` (`sim_physics_dump.cpp:389`) — no audio device.
   - a **base** `StatsRecorder` via `make_shared<StatsRecorder>()` (`sim_physics_dump.cpp:391-396`):
     a default `Game` has a null `stats_recorder`; once the damage path runs, `DamageDealt` would
     deref it and crash headless. The base class is a pure no-op; the *crashing* subclass is
     `NormalStatsRecorder`. **This is the 5b StatsRecorder trap — use the base, never Normal.**
   - **No viewports, no renderer, no SDL.** `WideRollbackChecksum` folds no viewport/`screen_flash`
     state, and `RecordFrame` does not draw, so the generator needs none of the render wiring 4d's
     `render_live` required. (`ProcessViewports` over an empty `game.viewports` is a no-op.)
4. `ReplayWriter` over a `DeflateWriter`→`FileWriter` sink: `BeginRecord(game)` (writes magic +
   version + the cereal `Game`, `replay.cpp:148-163`), then per tick — set each worm's
   `control_states` from the scenario input, `RecordFrame()` **then** `game.ProcessFrame()` (the
   `LocalController` order, input-map §1b/§4), for enough ticks (**> 1050**) that at least one
   `WideRollbackChecksum` word is embedded. `EndRecord` (`0x83`) fires in the writer's destructor.

Because the generator drives the **real** `Game::ProcessFrame`, the embedded checksums are
authoritative C++ engine output; Rust reproduces that trajectory from the same scenario. A sanity
step: play each generated `.lrp` back through the C++ `ReplayReader` / `framehash` (`CMakeLists.txt:552`)
to confirm it is a valid replay before committing.

**Corpus contents (committed under `golden/` beside the scenarios):** a handful of small `.lrp`
fixtures over **existing** scenarios, at least one **> 1050 ticks** (to exercise the checksum), plus
short ones for the empty-frame / end-tag / multi-worm-order paths. **Legacy `version < 7` files
cannot be generated** by the current writer (it only emits `kMyReplayVersion`); legacy branches are
covered by small **hand-crafted byte fixtures** in unit tests (or deferred to Phase 2, since the
legacy expansions are palette-only and Phase-1 skips all cereal/settings).

The generation itself is a **C++-mirroring task** (clang-format-22 + clang-tidy clean, one
`add_executable` block, `gen_*.sh` producing the fixtures). Once committed, the Phase-1 gate needs
**no C++ at test time** — self-checking like 4b.

---

## 4. Deflate dependency choice

**Use `flate2`** (`flate2::read::ZlibDecoder`) with its default pure-Rust `miniz_oxide` backend.
`flate2 1.1.9` + `miniz_oxide 0.8.9` are **already in `rust/Cargo.lock`** (transitively via `png`),
so this adds no new third-party surface and stays wasm-safe (no C dep). Add `flate2` as a *direct*
dependency of the new `replay` crate.

**Empirical unknown to pin (T-level RED):** miniz's `mz_deflateInit` default emits a **zlib-wrapped**
stream (2-byte header + adler32), so `ZlibDecoder` is the expected match; a raw-deflate producer would
need `DeflateDecoder`. Rather than reason about miniz internals, the reader's first test decompresses
a **committed generated `.lrp`** and asserts round-trip length/first-bytes — if `ZlibDecoder` fails,
fall back to `DeflateDecoder`. We only need to *decompress* C++'s output (any conformant inflate
suffices); we never reproduce C++'s *compression*, so byte-exact deflate framing on our side is a
non-issue.

---

## 5. Where the Rust code lives; gate construction per phase

**New crate `rust/replay`** (Bevy-free, like `scenario`/`sim`): owns the `.lrp` *container + delta
stream* reader — magic/version validation, inflate-to-memory, cereal-blob skipping, tag framing,
per-worm XOR decode, and checksum-word extraction. It produces, from a `.lrp`'s bytes, a decoded
per-tick `Vec<[ControlState; N]>` plus a `cycle -> u32` map of embedded checksum words. Rationale:
`.lrp` is the *foreign* format we only read (overview §Open Q6 — "two formats by design"); keeping it
out of `scenario` (our native format) is deliberate. Depends on `sim` (for `ControlState`,
`wide_rollback_checksum`), `flate2`.

**The `WideRollbackChecksum` port lives in `sim`** (`sim/src/wide_checksum.rs`) — it folds `SimState`,
a second hash parallel to `hash_game_state`. Keeping it in `sim` lets both the gate and any future
rollback (Step 5) reuse it.

**Phase-1 gate** — `rust/oracle-tests/tests/replay_lrp_phase1.rs` (has `scenario` + `sim` + `replay`
on hand, like the `render_slice4d` tests):

1. Reconstruct tick-0 `SimState` from the source scenario via `scenario::loader::load`.
2. Decode the committed `.lrp` with `replay` into the per-tick input stream + checksum map.
3. Replay: each tick, apply the decoded `[ControlState; N]` to the sim's worms, `process_frame`; at
   every `cycles % 1050 == 0` boundary compute `wide_rollback_checksum(&state)` and assert it equals
   the embedded word (folded at the same loop point C++ uses, §1).
4. **Cross-check** the Rust `hash_game_state` series against the scenario's committed `_sim.txt`
   golden — proving the decoded inputs reproduced the *same* inputs the sim golden ran (surfaces any
   divergence at the **first** differing tick, not at 1050). **This is the milestone gate: the corpus
   plays back bit-exact.**
5. **Non-vacuity:** assert ≥1 checksum word was actually consumed (corpus > 1050); a **negative**
   test corrupts one embedded word and asserts the gate *fails* (proving it is not a no-op).

**Phase-2 gate (deferred)** — cereal `Game` parse → drive sim → diff the per-frame render `framehash`
(`CMakeLists.txt:552`) `<tick> <fnv>` + `total` over arbitrary real `.lrp`. Needs the render path +
cereal graph; out of Phase-1 scope.

---

## 6. Risks & the hard 10%

1. **`WideRollbackChecksum` port fidelity + state-inventory gaps (the desync-at-1050 trap).** Rust
   must fold fields `HashGameState` omits — `worm.flags` (constant `0` in the KillEmAll corpus),
   reader-tracked `prev_control_states`, `aiming_speed/killed_timer/current_frame/current_weapon/
   direction/ready/able_to_*`, per-weapon triples, `sobject.x/y`, `wobject.owner_idx`,
   `bonus.frame/timer`, the whole material buffer — in the **exact** `Mix32` order
   (`replay.cpp:181-257`). One mis-ordered/missing field desyncs **only** at the 1050 cadence.
   Mitigate: line-for-line port + hand-folded unit test; corpus > 1050 ticks; negative test.
2. **Tick-0 alignment between the C++ generator's `Game` and Rust's scenario-loaded `SimState`.** The
   checksum folds the entire state; any tick-0 difference (weapon resolution, blood-pool size, spawn
   consts, seed handling, worm order) diverges silently until frame 1050. Mitigate: the generator
   reuses the dumper's **exact** scenario→`Game` setup (the code path that already yields
   `HashGameState`-matching goldens) and the **real** `Game::ProcessFrame`; the Phase-1 gate
   cross-checks the `HashGameState` series against `_sim.txt`, surfacing any drift at the first tick.
3. **Deflate framing (zlib-vs-raw) + whole-stream inflate fidelity.** miniz's default output framing
   must match the Rust decoder (`ZlibDecoder` vs `DeflateDecoder`); a wrong pick fails or truncates.
   Mitigate: an empirical RED test decompressing a committed generated `.lrp` (fall back to
   `DeflateDecoder` if needed); inflate whole-to-memory mirroring C++. Deflate is only a container
   transform — we decompress C++'s output, never reproduce its compression.

Secondary: the **corpus generator is a headless C++ tool** subject to the `NullSoundPlayer` + **base**
`StatsRecorder` traps (§3) — using `NormalStatsRecorder` crashes; and legacy `version < 7` files can't
be generated by the current writer (covered by hand-crafted byte fixtures or deferred to Phase 2).

---

## 7. Next artifact

The TDD task plan: `plans/2026-07-13-liero-rs-step4-slice4e-plan.md`.
