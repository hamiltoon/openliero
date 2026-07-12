# Step 4 — C++ input / `.lrp` replay / audio / viewport: fact map

Status: **FACT MAP — C++-side ground truth for Step 4** · 2026-07-12
Part of: `2026-06-26-liero-rs-roadmap.md`
Feeds: `2026-07-12-liero-rs-step4-input-replay-overview.md` (the Step 4 architecture doc, cited as **overview §N**)
Sibling precedent: `2026-07-10-liero-rs-step3-cpp-render-pipeline-map.md` (the Step 3 render-map)

This is the just-in-time **map of the C++ engine's input sampling, `.lrp` replay format,
sound-triggering mechanism, and `ProcessViewports` (shake/flash/banner)** — the four C++
subsystems Step 4 must reproduce or interoperate with. Every claim is cited `file:line` against
the worktree's `src/`. It contains **no Rust design** (that is the overview); it establishes what
is true on the C++ side so the overview and the per-slice specs can be written against ground
truth. Determinism traps are flagged inline with ⚠.

Read together with what already exists on the Rust side: the `sim` crate's tick is
`SimState::process_frame(&[ControlState; N])` — **already a per-tick input-snapshot function**
(`rust/sim/src/state.rs`), `ControlState` already mirrors `worm.hpp` bit-for-bit
(`rust/sim/src/state.rs:54-120`, 7-bit pack/unpack `&0x7f`), and the `game` binary already ticks
it on Bevy `FixedUpdate` at `1000/14 Hz` off a scenario's **recorded** per-tick inputs
(`rust/game/src/main.rs:104-109,244-261`). Step 4 replaces the *source* of those inputs (scripted
→ live keyboard / recorded / `.lrp`) and adds the side-effect surfaces (audio, live viewport).

---

## §1 — Input pipeline: SDL event → `control_states` → tick

### §1a The event path (there is **no** separate per-tick sample)

C++ input is **event-driven and edge-written into live worm state**, not sampled once per tick:

- `Gfx::ProcessEvent` (`gfx.cpp:596`) handles `SDL_EVENT_KEY_DOWN` / `SDL_EVENT_KEY_UP`:
  - down (`gfx.cpp:598-611`): `SDLToDOSKey(scancode)` (`keys.cpp:70`) → sets `dos_keys[kDosScan]=true`
    and, **only when `!ev.key.repeat`** (⚠ hardware key-repeat is filtered out here),
    `controller->OnKey(kDosScan, true)` (`gfx.cpp:608-610`).
  - up (`gfx.cpp:631-641`): clears `dos_keys[...]` and always `controller->OnKey(kDosScan, false)`.
- `LocalController::OnKey` (`localController.cpp:58-86`): `game.FindControlForKey(key, control)`
  (`game.cpp:75`) maps a DOS scancode to `(worm, Worm::Control)` via each worm's key bindings, then:
  - `worm->clean_control_states.Set(control, key_state)` — the **physical** key state
    (`localController.cpp:61`);
  - if `control < kMaxControl` (a real 7-bit control), `worm->SetControlState(control, key_state)`
    — writes the **live** `control_states` bit the sim reads (`localController.cpp:64-66`);
  - ⚠ **the `Dig` special-case** (`localController.cpp:69-79`): `clean_control_states[kDig]` is not
    one of the 7 bits — when held it `Press(kLeft)+Press(kRight)`; when released it conditionally
    `Release`s left/right. Dig is a *synthetic* two-bit chord, not a stored bit. Any Rust key model
    must reproduce this expansion, not store a "dig" bit.
  - `kDkEscape` starts the fade-to-menu (`localController.cpp:82-85`).

The event handler mutates the worm's live `control_states` **immediately**; the fixed-cadence loop
then consumes *whatever bits are set at tick time*. So the "input for tick N" is the accumulated
edge state at the moment `ProcessFrame` runs — ⚠ **this is the single most important sampling fact:
sub-tick key toggles that cancel out before the tick are invisible; the model is level-triggered
per bit, not a queue of edges.**

### §1b Where the tick is driven, and the `prev_control_states` baseline

- Events are pumped in `Gfx::Process` (`gfx.cpp:727-733`): `while (SDL_PollEvent) ProcessEvent`.
  The play state calls `gfx->ProcessEvent` per event (`gamePlayState.cpp:16`) then
  `gfx->controller->Process()` (`gamePlayState.cpp:43`).
- `LocalController::Process` (`localController.cpp:122-202`) runs `kRealFrameSkip` ticks
  (`:154-155`): per tick, AI first (`:157-165`), `replay->RecordFrame()` **before**
  `game.ProcessFrame()` (`:166-176`) — ⚠ **recording happens before the tick advances**, so the
  recorded delta is against the previous tick's `prev_control_states`.
- At the **end** of every `ProcessFrame`: `worm->prev_control_states = worm->control_states`
  (`game.cpp:466-468`). This is the delta baseline the `.lrp` format XORs against (§3c) and is
  itself hashed (`replay.cpp:205-206`).

### §1c Cadence and frame-skip

- **Real-time cadence** is `kDelay = 14U` ms (`gfx.cpp:1176`), busy-wait/`SDL_Delay` to
  `last_frame + 14` (`gfx.cpp:1178-1189`) ⇒ **~71.43 Hz** (`1000/14`). The Rust `game` binary
  already uses `Time::<Fixed>::from_hz(1000.0/14.0)` (`rust/game/src/main.rs:107`).
- **Frame-skip** (a dev/replay fast-forward, keys `1`..`0` in `commonController.cpp:9-34`):
  `frame_skip` runs N ticks per rendered frame; `inverse_frame_skip` runs one tick every N frames
  (`localController.cpp:154`). ⚠ This is a *presentation* speed control — it changes ticks-per-
  rendered-frame, **not** the per-tick input semantics; determinism is by tick count, never wall
  clock. Not needed for parity, but the loop must not couple input sampling to render rate.

### §1d Focus / pause traps ⚠

- `Unfocus`/`Focus` (`localController.cpp:90-120`): when the controller loses focus it stops
  receiving key events; on regaining focus the replay writer re-checks settings hashes
  (`ReplayWriter::Focus`, `replay.cpp:382-392`) — held keys are **not** re-sampled, so a key held
  across a focus-loss boundary can desync a naive live→replay if the model re-reads OS key state
  instead of trusting the edge stream. The recorded `control_states` are the truth, not the OS.
- `paused{true}` on `Game` (`game.hpp:132`) — the sim starts paused; `StartGame` clears it. The
  weapon-selection phase (`kStateWeaponSelection`) runs its own key-repeat emulation
  (`localController.cpp:124-152`, `kKeyRepeatInitial`/`kKeyRepeatInterval`) because SDL repeat
  events are filtered (§1a) — this only matters if Step 4 ports weapon selection.

---

## §2 — `ControlState`: the 7-bit packed input (`worm.hpp:139-176`)

- Controls (`worm.hpp:139-148`), bit index = the `WormSettings` enum value:
  `kUp, kDown, kLeft, kRight, kFire, kChange, kJump`, `kMaxControl = 7`.
- `Pack()` returns `istate` **unmasked** (`worm.hpp:155-157`; the 7-bit mask is commented out).
- `Unpack(state)` stores `istate = state & 0x7f` (`worm.hpp:159`).
- `operator[]`, `Set`, `Toggle` are plain bit ops (`worm.hpp:163-173`).
- ✅ Already ported bit-exact in Rust: `ControlState(u32)`, `pack` unmasked, `unpack` `&0x7f`
  (`rust/sim/src/state.rs:61-120`). **No Step-4 work needed on the packed representation itself** —
  only on the *source* that fills it.

---

## §3 — The `.lrp` replay format (`replay.hpp`, `replay.cpp`)

### §3a Container

- Whole stream is **deflate-compressed**: writer is `io::DeflateWriter` (`replay.hpp:28`); reader
  `io::InflateReader` inflates the **entire** file into a `std::vector<uint8_t> data` up front
  (`replay.cpp:99-110`) — ⚠ the full inflate is deliberate so `R` can rewind to the recorded start
  (`replay.hpp:58-61`, `replayController.cpp:56-59`).
- Header: `uint32 magic = 'LRPF'` (big-endian pack `('L'<<24)|('R'<<16)|('P'<<8)|'F'`,
  `replay.cpp:112`), then a **single version byte** `kMyReplayVersion` (`replay.cpp:120,150`).
  Reject `version > kMyReplayVersion` (`replay.cpp:121-123`).
- Then the **initial `Game`**, cereal `PortableBinaryOutputArchive` of the whole `Game` object
  (`replay.cpp:129` read / `:152` write, via `CerealWrite`/`CerealRead` `replay.cpp:60-87`). ⚠
  **This is the large surface**: reading a real `.lrp` byte-faithfully means deserializing the
  entire `Game` cereal graph (`serialization/cereal_types.hpp`), gated by
  `g_cereal_replay_version` (`replay.cpp:128-130`). This is the hard 10% of `.lrp` interop.

### §3b Per-frame stream framing (`replay.cpp:19-27`, tags in the anon namespace)

Bytes `< 0x80` are worm-state deltas; bytes with bit 7 set are tagged records:
- `0x80 kReplayTagEmptyFrame` — a frame with **no input change** (all worms keep last input).
- `0x81 kReplayTagSettings` — a cereal `Settings` blob follows (`replay.cpp:271-279`).
- `0x82 kReplayTagWormSettings` — `uint32 worm_idx` + cereal `WormSettings` (`replay.cpp:280-289`).
- `0x83 kReplayTagEnd` — end of stream (`replay.cpp:290-292`).

### §3c Per-worm delta encoding (⚠ load-bearing)

- Write (`ReplayWriter::RecordFrame`, `replay.cpp:328-373`): if any worm's `control_states !=
  prev_control_states` (or `worms.size() <= 3`, always for the 2-worm case, `replay.cpp:339-340`),
  write **one byte per worm** = `Pack() ^ prev_control_states.Pack()` (`replay.cpp:359`),
  `assert(kState < 0x80)`; else write `0x80` empty-frame.
- Read (`ReplayReader::PlaybackFrame`, `replay.cpp:260-326`): the first byte `< 0x80` is worm-0's
  delta; subsequent worms read one byte each (`replay.cpp:293-307`). Per worm:
  `control_states.Unpack(state ^ prev_control_states.Pack())` (`replay.cpp:304`). Sequencing is
  **implicit in the frame loop** — no per-worm index in the stream. ⚠ Reproducing this requires the
  exact worm iteration order and the `prev_control_states` state machine (§1b).

### §3d Embedded desync checksum

- Every `cycles % (70*15) == 0` (i.e. **1050 frames**), a `uint32 WideRollbackChecksum` is written
  (`replay.cpp:369-372`) and, on playback, read and compared — throwing `"Replay has desynced"`
  on mismatch (`replay.cpp:317-323`). ⚠ A byte-faithful reader must consume this word on the exact
  cadence or the stream desyncs immediately after frame 1050.

### §3e Version / legacy handling

- `version < 7`: level palette stored as 6-bit VGA — expand `<<2` (`replay.cpp:132-138`); worm rgb
  `(v&63)<<2` via `ExpandLegacyWormRgb` (`replay.cpp:30-55,139`). `Settings`/`WormSettings` read
  mid-stream also legacy-expanded (`replay.cpp:273-278,285-287`). ⚠ Version-gated branches must be
  reproduced for old files; new files (`kMyReplayVersion`) skip them.
- The custom-palette flag is not stored; it is re-derived post-read (`replay.cpp:142-143`).

---

## §4 — Record / playback control flow

- **Record** (single-player only): `LocalController::ChangeState → kStateGame` opens a
  `ReplayWriter` at `Replays/<timestamp> <names>.lrp` iff `Settings::kExtensions &&
  record_replays` (`localController.cpp:237-267`), calls `BeginRecord` (writes header + cereal
  Game, `replay.cpp:148-163`), and `RecordFrame()` each tick **before** `ProcessFrame`
  (`localController.cpp:166-176`). `EndRecord` writes `0x83` in the destructor
  (`replay.cpp:91-97,165`).
- **Playback**: `ReplayController` (`replayController.cpp`). `Focus` → `replay->BeginPlayback`
  (reconstruct Game from cereal, `replayController.cpp:37`), then per tick
  `replay->PlaybackFrame(renderer)` **before** `game->ProcessFrame()` (`replayController.cpp:63-84`).
  `R` rewinds to `initial_reader_pos` (`replayController.cpp:56-59`). ⚠ `PlaybackFrame` returning
  `false` (tag `0x83`) ends the replay; the sim still `ProcessFrame`s that tick.
- ⚠ **Order asymmetry**: record writes the delta from the *previous* tick then advances; playback
  applies the delta then advances. Both keep `prev_control_states` via `ProcessFrame`'s tail
  (`game.cpp:466-468`). A Rust reader must interleave `apply-delta → process_frame → (prev updated
  inside)` identically.

---

## §5 — `WideRollbackChecksum` (`replay.cpp:177-258`) — distinct from `HashGameState`

The `.lrp`/rollback desync checksum is **not** the Step 2/3 `HashGameState`. It folds, in this
exact order (`replay.cpp:181-257`): `rand.last`; `cycles`; per worm — `pos.x/y`, `vel.x/y`,
`aiming_angle`, `aiming_speed`, `health`, `lives`, `kills`, `timer`, `killed_timer`,
`current_frame`, `current_weapon`, `direction`, `flags`, `visible`, `ready`, `able_to_jump`,
`able_to_dig`, `control_states.istate`, `prev_control_states.istate`, per-weapon
`ammo/delay_left/loading_left`; wobject/sobject/nobject/bonus pool counts + per-object fields; and
the whole `level.material_id` buffer. Mixed via `Mix32` (`replay.cpp:168`). ⚠ For `.lrp`
byte-faithful playback the reader must compute *this* hash (to consume/verify the embedded word),
which is a **second** checksum to port alongside `HashGameState`. Rust already carries all these
fields (they are the same rollback-state inventory the sim hashes); only the mixing function +
field order differ.

---

## §6 — Sound triggering (⚠ the speculative flag; RNG-vs-output split)

There is **no sound queue**. Sim code calls `game.sound_player->Play(sound, id, loops)` **inline**
during `ProcessFrame`, scattered across the entity `Process` methods. `SoundPlayer::Play`
(`mixer/player.hpp:15-24`) early-returns when `speculative` is true (`game.hpp:97-109` sets it on
`sound_player`/`stats_recorder` together). Looping sounds are keyed by a `void* id` (usually an
object/weapon pointer) with `IsPlaying(id)`/`Stop(id)` (`player.hpp:26-27`). ⚠ `Stop` is **not**
gated by speculative (`player.hpp:29-38` comment: a suppressed stop leaks a looping channel).

Representative call sites (all inside the tick): worm bump `SoundBump` (`worm.cpp:175,188`), reload
`SoundReloaded` (`worm.cpp:309,833`), weapon loop start/stop keyed by `&weapons[current_weapon]`
(`worm.cpp:340-341,360-361,374-379,1075-1076,1119-1124`), respawn `SoundAlive` (`worm.cpp:789`),
ninjarope throw (`worm.cpp:979`); plus sobject explosion sounds and nobject hit sounds.

⚠ **The critical split, and the clean seam for Step 4 audio:** the `rand()` draws that *select
which sound variant* are part of the sim and are **already reproduced** by the Rust sim — e.g.
sobject `rand(num_sounds)` iff `start_sound >= 0` (`sobject.cpp:24`; ported/consumed in
`rust/sim/src/sobject.rs:15-17`, "the `Play` is a hashing no-op but the `rand` is consumed"),
and the hit-sound gate `rand(3)` (ported in `rust/sim/src/nobject.rs:342-347,537-574`). Only the
`Play()` *output* is omitted in Rust today (grep `"sound not hashed"` across
`rust/sim/src/{control,nobject,sobject}.rs`). So audio is **purely additive**: the sim can emit a
per-tick sound-event record (sound id / already-chosen variant / object handle / loop flag)
**without adding or moving a single `rand`**, and `speculative` maps to "don't emit events on
predicted/resim ticks." No RNG or hashed state is touched.

---

## §7 — `ProcessViewports` + top-of-frame decrements (shake / flash / banners)

These are the render-only side effects Step 3 deliberately skipped (3b/3c/3e deferrals). They live
**inside `ProcessFrame`** but touch no hashed sim state except `screen_flash`.

### §7a Top-of-`ProcessFrame` decrements (`game.cpp:271-332`)

- `if (screen_flash > 0) --screen_flash` (`game.cpp:271-273`). ⚠ `screen_flash` **is** snapshot
  state (`game.cpp:693,762`) — it is sim state, must roll back, and drives the palette `LightUp`
  at draw (`game.cpp:179-180`). It is set by explosions (sobject) elsewhere.
- Per (spectator) viewport: `if (shake > 0) shake -= 4000` (`game.cpp:275-285`). `shake` is a
  **viewport** field (render-only, not hashed).
- Banner stepping, only on `(cycles & 1) == 0` (`game.cpp:292-332`): `banner_y` walks toward `2`
  when the viewport's worm `killed_timer > 16`, else toward `-8`. `banner_y` is viewport-only
  (render). ⚠ It is stepped every *other* cycle — a parity subtlety if banners are ported.

### §7b `ProcessViewports()` (`game.cpp:463`, `viewport.cpp:22-76`) — called near end of frame

Per viewport `Process` (`viewport.cpp:22-76`):
- Centering: if alive+visible, `SetCenter` on the steerable centroid
  (`steerable_sum_x/steerable_count`, `worm.hpp:267`) when `steerable_count > 0`, else on
  `Ftoi(worm.pos)` (`viewport.cpp:28-35`); if invisible, `ScrollTo(..., 4)` (`:36-38`); if dead,
  `SetCenter` on pos and set `banner_y=-8` at `killed_timer==kKilledTimerInitial` (`:39-45`).
  ⚠ **Rust-state gap**: `WormState` has `steerable_count` (`rust/sim/src/state.rs:342`) but **no
  `steerable_sum_x/y`** (the 3b assert flagged this) — steerable camera centering can't be ported
  faithfully until those two accumulators are added to the sim state (they are set inside the
  steerables loop `worm.cpp`).
- Shake RNG: `if (Ftoi(shake) > 0) { x += rand(kRealShake*2)-kRealShake; y += ... }`
  (`viewport.cpp:47-52`) using the **viewport-local `Rand`** (render-map §6, the default-seeded
  `rand.hpp` RNG the render crate already reproduces for laser sparks in 3b), **never** `game.rand`.
  Then clamp to `[0, max]` (`viewport.cpp:54-57`).

⚠ Because centering/shake/banner read only sim outputs (worm pos/health/killed_timer/screen_flash)
and mutate only viewport-local state + the viewport-local RNG, the entire `ProcessViewports` port
is a **render/game-layer** concern — it does not belong in the Bevy-free determinism firewall, and
wiring it live cannot perturb `HashGameState` (the Step 3 isolation invariant still holds).

---

## §8 — Determinism-trap summary (the checklist for the overview)

1. ⚠ Input is level-triggered per bit, mutated live by events, consumed at tick time — **not** a
   sub-tick edge queue (§1a). The recorded/`prev_control_states` stream is the source of truth,
   not OS key state (survives focus loss).
2. ⚠ `Dig` is a synthetic left+right chord, not a stored bit (§1a).
3. ⚠ Hardware key-repeat is filtered (`!ev.key.repeat`) (§1a); weapon-select re-emulates repeat.
4. ⚠ `prev_control_states` is updated in `ProcessFrame`'s tail (`game.cpp:466-468`) — the delta
   baseline; both record and playback depend on it (§1b, §3c).
5. ⚠ `.lrp` initial state is a **whole-`Game` cereal graph** — the large interop surface (§3a).
6. ⚠ `.lrp` embeds a `WideRollbackChecksum` every 1050 frames — a *second* hash to port, consumed
   on an exact cadence (§3d, §5).
7. ⚠ Audio: the sound-*selecting* `rand` is sim state (already ported); the `Play` *output* is not
   — audio is additive and must add zero rand draws (§6). `speculative` gates output only.
8. ⚠ `screen_flash` is hashed sim state; `shake`/`banner_y` are viewport-only render state (§7).
9. ⚠ Shake uses the viewport-local RNG, never `game.rand` (§7b). Steerable centering needs two
   sim-state accumulators not yet in Rust `WormState` (§7b).
</content>
</invoke>
