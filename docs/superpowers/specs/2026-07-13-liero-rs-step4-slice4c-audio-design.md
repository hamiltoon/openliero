# Step 4, Slice 4c — Audio: design

Status: **DESIGN — Step 4 slice 4c** · 2026-07-13 · not started
Part of: `2026-06-26-liero-rs-roadmap.md`
Detailing: the **4c** bullet of `2026-07-12-liero-rs-step4-input-replay-overview.md` (cited **overview §N**)
Grounded in: `2026-07-12-liero-rs-step4-cpp-input-replay-map.md` §6 (cited **input-map §6**)
Sibling precedent: `2026-07-12-liero-rs-step4-slice4b-record-replay-design.md`

This is the per-slice design for **audio**. It fixes the sim-emitted sound-event stream, the full C++
callsite inventory and its Rust targets, the loop-sound lifecycle, the audio backend, the wasm embed
decision, and the isolation-gate construction. Companion plan:
`plans/2026-07-13-liero-rs-step4-slice4c-plan.md`.

---

## 1. Scope and what it proves

Audio is a **pure consumer of a sim-emitted event stream** (overview Locked decision 3). The sim
already draws every RNG that *selects* a sound variant (input-map §6); 4c adds only an **event
record** at each existing `Play`/`Stop` callsite and a `game`-side backend that drains it. **Zero new
`rand`. Zero hashed-state change.** The sim stays Bevy-free and its `hash_game_state` byte-identical.

**Proves:** a live match makes the right sounds (native + wasm); the determinism firewall is untouched.
**Gate:** `hash_game_state` byte-identical across every committed golden (isolation, standing since
Step 3); the 4b record→replay round-trip stays green; audio *output* is advisory (eyeball/ear, never CI).

Depends only on 4a (a live loop). Independent of 4d — **do not** touch 4d files, `PROGRESS.md`'s 4d
line, or the overview's 4d bullet.

---

## 2. The event model and stream API

### 2.1 The event type (new `sim` module `sound.rs`, Bevy-free)

The variant is **already chosen inside the sim** at every callsite, so every event carries a *final*
resolved index into `sounds[]` — the game never re-draws or re-resolves.

```rust
// rust/sim/src/sound.rs  (no Bevy, no f32)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAction { Play, Stop }

/// A stable identity for a LOOPING sound channel. Mirrors the C++ `void* id`
/// (input-map §6) — the pointer key that `IsPlaying`/`Stop` use — but as a
/// value key the game can hash. One-shots carry `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoopKey {
    Worm(u8),            // &worm / `this` (worm.cpp:360-361, weapon.cpp:311, nobject/sobject loops)
    WormWeapon(u8, u8),  // &weapons[current_weapon] (worm.cpp:1120-1121, Stop at :341/:375/:1076)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundEvent {
    /// Final index into `sounds[]` (>= 0). Variant ALREADY chosen in-sim
    /// (start_sound + rand(num_sounds), 15+rand(3), 18+rand(3), explo/launch_sound,
    /// or a resolved sound_hook). Hook resolution happens at the callsite via
    /// `config.sound_hooks`, so the game sees one uniform kind: an index.
    pub sound: i32,
    /// `None` => one-shot. `Some(key)` => looping channel identity.
    pub key: Option<LoopKey>,
    pub action: SoundAction,
}
```

Rationale for a **resolved index, not a hook enum**: the callsites are heterogeneous (hooks, object
`start_sound`, `15+rand(3)`, `explo_sound`, …). Resolving to an index at emit time makes the drain
side a trivial `sounds[ev.sound]` lookup and keeps `sound_hooks`/`config` entirely on the sim side,
where it already lives (`assets::SoundHooks`, `SObjectType::{start_sound,num_sounds}`,
`Weapon::{launch_sound,explo_sound,loop_sound}`).

### 2.2 The stream: a per-tick `Vec` on `SimState`, cleared at top, drained by `game`

```rust
// SimState (rust/sim/src/state.rs) — NEW field, NOT hashed:
pub sound_events: Vec<SoundEvent>,
```

- `process_frame` **clears** `self.sound_events` at the very top (before any sim work), so the vec
  holds exactly this tick's events — mirroring C++ inline `Play` (there is no queue; sounds fire
  during the tick — input-map §6). Only the last tick's events ever survive; nothing accumulates.
- Every ported callsite pushes a `SoundEvent` at the point where C++ calls `Play`/`Stop`.
- The `game` layer, in `tick_and_render` **after** `process_frame`, drains `&sim.0.sound_events` into
  the backend (`main.rs:420`, right after the `process_frame` call). Headless/library callers
  (`replay_state_series`, the 4b round-trip, the passthrough gate) simply **ignore** the vec — no sink,
  no sound. That is the whole isolation story (see §6).

**Why a field on `SimState`, not an out-param to `process_frame`:** keeping the `process_frame(&[Control
State; N])` signature *unchanged* preserves the ggrs-shaped snapshot boundary (overview Locked
decision 1) and the 4a/4b call sites. A drained-per-tick vec is the least invasive seam. The field is
`#[allow]`-clear of the hash: `hash_game_state` is a hand-written field walk (`sim/src/hash.rs`) that
simply never reads `sound_events` — the isolation invariant is *structural*, proven by the unchanged
golden hashes (§6), exactly as `screen_flash` vs `shake` split render state in Step 3.

### 2.3 Speculative / resim — not needed in Step 4, but the model is forward-safe

The Rust sim has **no `speculative` flag** and Step 4 has **no resim** (Scripted reloads by rebuilding
the state; Replay only advances; there is no rollback until Step 5). So 4c needs no speculative gate:
isolation is achieved by *the sink only existing in `game`*, and the vec being drained only on the real
interactive tick. The C++ `speculative` early-return in `SoundPlayer::Play` (input-map §6) maps, in
Step 5, to "don't **emit** Play events on predicted/resim ticks" — a one-line guard around the pushes,
added when rollback lands. Crucially, C++ does **not** speculative-gate `Stop` (a suppressed stop leaks
a channel — input-map §6); the event model preserves this by making `Stop` events unconditional. This
is documented here as a **forward hook**, explicitly **out of 4c scope**.

---

## 3. Full callsite inventory (C++ `file:line` → Rust target → event)

All callsites below are **inside `ProcessFrame`** (the sim path). Menu/UI sounds
(`mainMenuState.cpp`, `weaponMenuState.cpp`, `weapsel.cpp`, `gfx.cpp:167/173/208/1194`, `menu/*`,
`inputState.cpp`, `fileSelectorState.cpp`, `rematchState.cpp`, `rollbackController.cpp:430/437`) go
through `g_sound_player` from the **UI flow**, not the sim — they are **out of scope** (no menu in
Step 4; overview §Open Q4). The Rust "sound not hashed / omitted" markers already pin each site.

### 3.1 One-shot, `sound_hook`-resolved

| C++ site | sound | Rust target (existing omit-marker) | Event |
|---|---|---|---|
| `worm.cpp:175` | `SoundBump` | `sim/src/physics.rs` worm-bump (bounce X) | `Play` idx=`sound_hooks.Bump`, key=None |
| `worm.cpp:188` | `SoundBump` | `sim/src/physics.rs` worm-bump (bounce Y) | same |
| `worm.cpp:309` | `SoundReloaded` | `sim/src/control.rs:471` reload-complete | `Play` idx=`sound_hooks.Reloaded` |
| `worm.cpp:833` | `SoundReloaded` | bonus/full-ammo reload — `sim/src/bonus.rs:533` | `Play` idx=`sound_hooks.Reloaded` |
| `worm.cpp:789` | `SoundAlive` | `sim/src/state.rs:2583` respawn (`AfterSpawn`) | `Play` idx=`sound_hooks.Alive` |
| `worm.cpp:979` | `SoundNinjaropeThrow` | `sim/src/control.rs:342` ninjarope throw | `Play` idx=`sound_hooks.NinjaropeThrow` |
| `game.cpp:512` | `SoundBegin` | *(no `StartGame` port; match starts pre-initialised)* | **DEFERRED — see §7** |

### 3.2 One-shot, object-`start_sound` + already-drawn variant

| C++ site | sound | Rust target | Event |
|---|---|---|---|
| `sobject.cpp:24` | `rand(num_sounds)+start_sound` | `sim/src/sobject.rs:142-143` (rand already drawn) | `Play` idx=`start_sound + <drawn>`, key=None |
| `weapon.cpp:94` | `w.explo_sound` (wobject explode) | `sim/src/weapon.rs:744` | `Play` idx=`explo_sound` |
| worm death spray | `15 + rand(3)` | `sim/src/state.rs:2706-2707` (rand already drawn) | `Play` idx=`15 + <drawn>` |
| worm hit/blood | `18 + rand(3)` | `sim/src/state.rs:2642-2645` (rand already drawn) | `Play` idx=`18 + <drawn>` |

Note the death-spray / hit indices `15+`/`18+` are **hardcoded sound-table offsets** in the sim (the
draw is already there); the event just captures the number the sim computed.

### 3.3 One-shot, weapon `launch_sound` (non-loop fire)

| C++ site | sound | Rust target | Event |
|---|---|---|---|
| `worm.cpp:1124` | `w.launch_sound` (non-loop branch) | `sim/src/weapon.rs:204` launch-sound skip | `Play` idx=`launch_sound`, key=None |

### 3.4 Loop sounds — keyed by object handle (Play + Stop)

| C++ site | action | key | Rust target | Event |
|---|---|---|---|---|
| `worm.cpp:1120-1121` | `Play(launch_sound, &weapons[cur], -1)` | `WormWeapon(worm, cur)` | `sim/src/weapon.rs` fire loop | `Play` idx=`launch_sound`, key=`WormWeapon` |
| `worm.cpp:341` | `Stop(&weapons[cur])` | `WormWeapon` | `sim/src/control.rs:568` loop-stop | `Stop` key=`WormWeapon` |
| `worm.cpp:375` | `Stop(&weapons[cur])` | `WormWeapon` | fire-cease / weapon-switch | `Stop` key=`WormWeapon` |
| `worm.cpp:1076` | `Stop(&weapons[cur])` | `WormWeapon` | `sim/src/control.rs:568` | `Stop` key=`WormWeapon` |
| `worm.cpp:360-361` | `Play(kSnd, this)` | `Worm(idx)` | worm fire loop | `Play`, key=`Worm` |
| `worm.cpp:379` | `Play(kDeathSnd, this)` | `Worm(idx)` | `sim/src/state.rs` death path | `Play`, key=`Worm` |
| `weapon.cpp:311-312` | `Play(kSnd, &worm)` | `Worm(idx)` | `sim/src/weapon.rs:618` hit-loop | `Play`, key=`Worm` |
| `nobject.cpp:183-184` | `Play(kSnd, &w)` | `Worm(idx)` | `sim/src/nobject.rs:570-574` hit-loop | `Play`, key=`Worm` |
| `sobject.cpp:108-109` | `Play(kSnd, &w)` | `Worm(idx)` | `sim/src/sobject.rs:249` | `Play`, key=`Worm` |

**Sizing:** ~13 `Play` sites (7 one-shot families + 6 loop) + ~3 distinct `Stop` sites, across **5
families**: (a) worm-hook one-shots (bump/reload/alive/ninja), (b) object-variant one-shots
(sobject/death/blood), (c) weapon one-shots (explo/launch), (d) worm-keyed loops (fire/hit/death), (e)
weapon-slot-keyed loops (launch loop + its 3 stops). Every site has an existing Rust omit-marker to
convert — no new sim search needed.

**C++ `IsPlaying` guard:** several loop plays are wrapped `if (!IsPlaying(id)) Play(...)`. The sim need
**not** model `IsPlaying` — it emits a `Play(loop, key)` every tick the loop should sound; the *game
backend* makes it idempotent (start iff not already playing on that key). This keeps the sim free of
audio-channel state (which is not deterministic and must not be).

---

## 4. Loop-sound lifecycle and the channel-leak risk

### 4.1 The backend model

The game holds `loops: HashMap<LoopKey, LoopHandle>`. Per drained event:

- `Play` + `key=Some(k)`: if `k` not in map, start a looping playback of `sounds[ev.sound]`, insert its
  handle. If already present, **no-op** (idempotent — reproduces the C++ `IsPlaying` guard).
- `Stop` + `key=Some(k)`: remove `k`, stop/drop its handle.
- `Play` + `key=None`: fire-and-forget one-shot (no map entry).

### 4.2 The leak (input-map §6 — the real hazard)

A looping channel that is started but never stopped **leaks forever** (finite mixer channels). C++
avoids this by (a) explicit `Stop` at fire-cease/weapon-switch/death, and (b) **not** speculative-gating
`Stop`. Our model preserves both. But two Rust-specific gaps must be closed:

1. **Object freed without a Stop.** If a worm dies or a weapon slot changes and the corresponding
   `Stop` site is not reached, the loop leaks. Mitigation: the sim emits `Stop` at exactly the C++ Stop
   sites (§3.4). The death path (`state.rs:2703-2707`) already notes the loop-sound stop is *omitted* —
   4c must **emit** the `Stop(Worm(idx))` / `Stop(WormWeapon(..))` there.
2. **Belt-and-braces reaper (recommended).** Because the sim owns object lifetimes and the game does
   not, the game additionally reaps any `LoopKey::Worm(i)` whose worm is dead/invisible and any
   `WormWeapon(i, w)` whose worm's `current_weapon != w`, read from the (already-available) sim state
   each tick. This makes a *dropped* Stop event self-heal instead of leaking — the closest safe analog
   to the C++ "a spurious stop self-heals, a suppressed stop leaks" comment. Cheap: iterate the small
   `loops` map, drop entries whose key is no longer live.

Design decision: **emit the explicit C++ Stops (correctness parity) AND run the liveness reaper
(robustness).** The reaper is not an RNG or hashed path — pure game-side.

---

## 5. Backend choice: **rodio** (native), behind a small `AudioSink` trait

**Recommendation: `rodio`**, native, with the backend hidden behind a `game`-local `AudioSink` trait
(`RodioSink` + `NullSink`) that mirrors C++ `SoundPlayer`/`NullSoundPlayer`.

Weighing (fresh check, 2026-07: neither kira nor rodio nor cpal is in `Cargo.lock` today — this is a
clean add either way):

| Criterion | rodio | kira |
|---|---|---|
| Raw-`i16`-buffer source | **`SamplesBuffer::new(1, rate, Vec<i16>)`** — an *exact* fit for `wav.rs::upsampled()` | `StaticSoundData` from raw frames; slightly more plumbing |
| One-shots (many simultaneous) | `stream_handle.play_raw` / a `Sink` per shot | `manager.play` per shot |
| Loop handles keyed by object | `Sink` per key in a map; `.stop()`/drop; `Sink::append(src.repeat_infinite())` | `StaticSoundHandle` per key; `.stop()`; first-class |
| Dep weight | lighter | heavier (game-audio framework) |
| Native backend | cpal | cpal |
| **Web backend** | cpal web-audio (community-supported, finicky: single-thread, AudioContext resume) | cpal web-audio (kira documents wasm, same underlying caveat) |

Both bottom out on **cpal** for the browser, so *neither* gives free web magic — the wasm caveat
(user-gesture AudioContext resume, single-threaded worklet) is shared. Given that parity, the
tie-breakers are: rodio's `SamplesBuffer<i16>` is a **one-line** fit to our already-golden
`WavSound::upsampled()` output, it is lighter, and the loop model (a `Sink` per `LoopKey`) is simple
enough. The **`AudioSink` trait seam** (justified independently by the headless `NullSink` need — every
test and `replay_state_series` uses it) contains the risk: if the rodio **wasm** path proves too
painful in T5, swapping the wasm `AudioSink` impl to **kira** is a single-file change, no sim/game-logic
churn. This realises overview §Open Q3's "thin, Bevy-free, swappable, not `bevy_audio`."

**Sample rate:** `wav.rs::upsampled()` yields a 2× linear-interpolated `Vec<i16>` from 22050 Hz source
(so present it to the backend at **44100 Hz mono**); `original_data` is the golden artifact,
`upsampled()` is the playback buffer. The backend consumes `upsampled()` directly.

---

## 6. Isolation-gate construction

The standing invariant (overview Oracle strategy, "Isolation gate"): after wiring audio,
`hash_game_state` is **byte-identical** on every committed golden, native and wasm. Construction:

1. **The event vec is never hashed.** `hash.rs` is a hand-written field walk; it does not read
   `sound_events`. A dedicated unit test builds a `SimState`, runs `process_frame` so `sound_events`
   is populated, and asserts `hash_game_state` equals the same run with events cleared — proving the
   field is hash-inert.
2. **No golden moves.** `git diff --stat golden/` after 4c must be empty except any *new* 4c fixtures —
   every `sim_slice*` / `render_slice3b_*` / `record_slice4b_*` golden re-diffs byte-identical.
3. **Zero new `rand`.** The variant draws are pre-existing (the omit-markers prove it); 4c adds only
   `Vec::push`. A test asserts `rand.draws()` for a representative tick is unchanged vs pre-4c.
4. **Headless is silent.** `replay_state_series`, the 4b round-trip, and the passthrough gate construct
   **no sink** (or a `NullSink`) — they read `hash_game_state`, never `sound_events`. The 4b hard gate
   stays green unchanged.

Because emission is unconditional `Vec::push` and the sink lives only in `game`, isolation is
*structural*, not a runtime flag — the strongest form.

---

## 7. Explicitly out of scope

- **Volume / panning / positional audio.** The C++ mixer has `SfxSetVolume` (`mixer.cpp:177`) but the
  **sim `Play` path never sets volume or pan** — `SoundPlayer::Play(sound, id, loops)` has no
  volume/pan argument and no callsite passes one. Playback is flat. 4c reproduces flat playback; no
  pan/volume.
- **`SoundBegin` (`game.cpp:512`).** Played by `StartGame`, which has no Rust port (matches start
  pre-initialised from a scenario). Deferred with 4f's start-flow, or a trivial one-shot at match start
  if 4f wants it — not 4c.
- **Menu / UI sounds** (`SoundMenu*` via `g_sound_player`). No menu in Step 4 (overview §Open Q4).
- **The C++ mixer itself** (`mixer/mixer.cpp` channel allocator, `SfxMixerMix`). We do **not** port the
  sample mixer — rodio/cpal mixes. We reuse only `wav.rs` (already ported) for sample bytes.
- **Speculative/resim gating.** No resim in Step 4 (§2.3); the Play-emit guard is a Step-5 forward hook.
- **`.lrp` audio / videotool.** Unrelated (4e / out of rewrite scope).

---

## 8. Risks (top 3)

1. **Loop-channel leak** (input-map §6, the named hazard). A loop started without a matching Stop leaks
   a channel. Mitigated by emitting the explicit C++ Stops **and** the game-side liveness reaper (§4.2);
   verified by a test that fires→ceases→dies and asserts the `loops` map returns to empty.
2. **Isolation regression via a "helpful" refactor.** The trap (overview Risks): moving a variant
   `rand` into the audio path. Prevented by keeping every draw in the sim (events carry the
   *already-computed* index) and by the hash-inert + `rand.draws()` tests (§6.1, §6.3).
3. **Wasm Web-Audio bring-up.** cpal's browser backend needs an AudioContext resumed on a user gesture
   and is single-threaded; the embedded wasm demo has no live input (3f), so only the scripted `blood`
   demo's sounds play — a narrow but real proof. Contained by the `AudioSink` seam (kira fallback) and
   by making T5 (wasm) the last, independently-revertable task so a wasm-audio stall never blocks the
   native milestone.

### Wasm embed decision

`sounds/` is **31 WAVs, ~505 KB raw** (`data/TC/openliero/sounds/`). Step 3f **excluded** `sounds/`
from the wasm embed (`scenario/src/assets.rs:34-35`, "audio is Step 4"). 4c **adds them**: embed the
raw `.wav` bytes via a new `include_dir!("$CARGO_MANIFEST_DIR/../../data/TC/openliero/sounds")` in the
wasm branch of `assets.rs`, decoded at startup by the existing `wav.rs`. +505 KB to the wasm binary is
acceptable (same order as the already-embedded sprites/level). **Do not** fetch over the network — 3f's
model is embed-everything, no runtime fetch. Curation (embed only demo-reachable sounds) is a possible
later size optimisation, **not** done now (the full set keeps a future live-wasm build correct and the
logic trivial).
