# Liero-rs — lightweight web + P2P netplay (EXPLORATION)

Status: **EXPLORATION — not a committed plan, not a spec** · 2026-06-27
Part of: `2026-06-26-liero-rs-roadmap.md` · informs: `2026-06-26-liero-rs-steps2-5-preliminary-breakdown.md` (step 3 wasm bring-up, step 5 rollback netplay)
Sibling: `2026-06-26-liero-rs-interactive-iteration-exploration.md` (the web build as an *observation* surface)

> **Read this first.** This document explores *how the web version of liero-rs can
> be lightweight*: (a) run the deterministic sim in a **Web Worker** off the UI
> thread, (b) serve the whole thing from a **CDN / static host** with at most a
> thin cloud backend, and (c) play multiplayer **peer-to-peer over WebRTC
> DataChannels** with no server-side game logic. It maps the design space and
> ends with a realistic assessment; it **commits nothing**. Exact APIs (Bevy,
> wasm-bindgen, wgpu, bevy_ggrs, matchbox) move fast and **must be re-researched
> just-in-time at steps 3 and 5** — the breakdown already flags "API churn
> (research-before-build)" for both. Everything here is at the strategy level.
>
> **Central observation:** OpenLiero already ships a complete rollback-netcode
> stack — ENet transport, libjuice ICE/STUN, a Go signaling/TURN-credential
> server, generation-tagged redundant input batches, and per-frame checksum
> desync detection. **The web version is a translation of that stack onto browser
> primitives, not an invention.** Almost every piece has a one-to-one WebRTC
> equivalent. This doc's job is to draw that map.

---

## TL;DR

- **The sim is deterministic and rollback exchanges only *inputs*, never state.**
  That single fact is what makes the web version lightweight: no authoritative
  server, no game state streamed over the wire, no per-frame server traffic. Two
  browsers exchange ~1 byte of input per frame, peer-to-peer, and each simulates
  locally — exactly what the C++ `RollbackController` + `NetTransport` already do.
- **Hosting collapses to: static CDN + a tiny signaling backend (+ optional
  TURN).** The wasm bundle and assets are static files (the C++ emscripten build
  already auto-deploys to gh-pages — proven precedent). The only always-on server
  is signaling, which forwards a handful of SDP/ICE messages at connect time and
  then goes silent. TURN is a rarely-needed relay fallback and the one place a
  recurring cloud cost can appear.
- **Run the sim in a Web Worker.** The fixed-tick deterministic loop (+ rollback
  schedule + RNG-as-state) belongs off the main thread so the UI stays
  responsive; rendering on the main thread or in the worker via OffscreenCanvas.
- **The hard constraints are wasm-threads vs hosting headers, and
  determinism-across-platforms.** SharedArrayBuffer/wasm-threads require
  COOP/COEP cross-origin isolation, which gh-pages cannot set — a real
  fork in the hosting road. And the crown jewel (bit-exact tick #1000) must
  survive the wasm target: native and wasm must produce identical checksums.
- **Minimal first milestone: single-player wasm in a worker on gh-pages
  (single-threaded).** The P2P milestone comes after, reusing the rollback work
  from step 5 with a WebRTC DataChannel behind GGRS.

---

## 1. The "lightweight" thesis — why *this* engine enables true P2P

Most browser multiplayer is heavy because the server is authoritative: it runs the
game logic, holds the truth, and streams state (or state deltas) to every client
every tick. That demands always-on compute that scales with player count and
match count, and it puts game logic in the cloud.

Liero-rs needs **none of that**, because of the crown jewel (`roadmap.md`:
"determinism is the crown jewel"). The simulation is a pure function of
`(state, input)`: fixed-point math, RNG-as-state, fixed iteration order, no
floats. Given the same seed, level, and per-tick input stream, every peer
computes bit-identical state. So the network never needs to carry *state* — only
the tiny *inputs* that each peer is missing. This is already true in the C++ code:

- **Rollback sends inputs, not state.** `RollbackController` emits "the last
  K = kMaxRollback + 1 local inputs" per send
  (`src/game/controller/rollbackController.hpp:20`), where each input is a single
  packed `ControlState` byte. With `kMaxRollback = 7`
  (`src/game/rollback/buffer.hpp:29`, "~100 ms of tolerance at 70 fps"), a full
  redundant input window is on the order of **a dozen bytes per send** —
  `[type:1][gen:1][baseFrame:4][count:1][localDelta:1][input[≤8]:8]`
  (`transport.hpp:53`). State snapshots (`GameSnapshot`, `fast_snapshot.hpp`) are
  saved and restored **locally** in the ring buffer (`buffer.hpp` `Slot`); they
  never go on the wire.
- **The server already holds no game state.** The Go server in `server/` is pure
  signaling: it pairs two peers by room code and forwards ICE credentials and
  candidates between them (`server/server.go`, `server/protocol.go`). It never
  sees a single frame of gameplay. Once the peers connect directly, the server is
  idle until the room times out (`roomTTL = 5 * time.Minute`, `server.go:18`).
- **Desync detection is already checksum-only.** Peers exchange a 4-byte
  per-frame checksum (`SendChecksum`, `transport.hpp:142`) and compare in a small
  ring (`NetSession::checksumBuffer_`, `session.hpp:180-193`). That is the entire
  "is the shared truth still consistent?" mechanism — no state reconciliation,
  because there is no shared state, only shared inputs and a hash that proves the
  independently-computed states agree.

**Contrast.** An "authoritative server streams state" design would mean: a
server-side build of the sim, per-match server processes, per-frame downstream
bandwidth, and a cloud bill that grows with concurrency. The deterministic +
rollback model deletes that entire column. The web translation inherits the
deletion for free — *as long as determinism holds on the wasm target* (§6).

This is the thesis: **because the engine is deterministic and rolls back on
inputs, the web version can be genuine peer-to-peer over WebRTC with no
server-side game logic and only a thin signaling backend.**

---

## 2. Web Worker architecture — the sim off the UI thread

### Why a worker

The sim is a fixed-rate, CPU-bound loop that must tick on a strict cadence and,
under rollback, occasionally re-simulate up to `kMaxRollback` frames in a burst
(predict → rollback → resimulate). If that runs on the browser's main thread it
competes with layout, input handling, and the event loop — causing jank and, worse,
letting UI stalls perturb tick timing. Putting the deterministic sim in a **Web
Worker** isolates the fixed-tick loop from the UI, the same separation the C++
engine gets for free as a native app with its own loop
(`emscripten_set_main_loop_arg` in the C++ wasm build is the single-thread
analogue — see the interactive-iteration doc §1a).

### The shape

```
┌─────────────── main thread (UI) ───────────────┐     ┌──── Web Worker ────┐
│  DOM / menu / input capture                     │     │  wasm sim module    │
│  WebRTC RTCPeerConnection + DataChannels        │◄───►│  sim-core (fixed,   │
│  (signaling, peer I/O)                          │ msg │   rng-as-state)     │
│  rendering: <canvas> (WebGL2/WebGPU)            │     │  rollback schedule  │
│         ── or ──                                │     │  per-frame checksum │
│  OffscreenCanvas handed to the worker           │◄────┤  (GgrsSchedule)     │
└─────────────────────────────────────────────────┘     └─────────────────────┘
```

Two viable splits, to decide at step 3 with fresh research:

- **Sim in worker, render on main thread.** Worker owns the authoritative state;
  each tick it posts a lightweight render-view (entity positions, dirty terrain
  cells) to the main thread, which draws. Clean isolation; cost is per-frame
  message marshalling of render data.
- **Sim + render both in the worker via OffscreenCanvas.** The main thread
  transfers an `OffscreenCanvas` to the worker (`canvas.transferControlToOffscreen()`),
  and the worker runs Bevy/wgpu rendering directly to it. The main thread only
  does DOM, input, and WebRTC. This is the cleanest "UI thread does almost
  nothing" design and the best fit for the lightweight goal — but it leans hardest
  on Bevy-in-worker maturity (§6).

Either way, **input flows main → worker** (keyboard/gamepad captured on the main
thread, posted as the per-tick `ControlState` the sim consumes — mirroring step
4's "input → 7-bit ControlState" model) and **remote input flows
WebRTC → main → worker** (or directly into the worker if the DataChannel is
opened there).

### wasm-in-worker mechanics, threads, and the COOP/COEP tax

- **wasm runs fine in a worker.** A worker can `fetch` + `instantiate` the wasm
  module and run it; this part is routine.
- **wasm *threads* (multi-threaded wasm) are the expensive option.** They require
  `SharedArrayBuffer`, which browsers gate behind **cross-origin isolation**: the
  document must be served with
  `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp` (COOP/COEP). This is a **hosting
  constraint with teeth** (§4): you must control response headers, and all
  cross-origin subresources must be CORP/CORS-clean. **gh-pages cannot set these
  headers**, so wasm-threads on gh-pages is only possible via a service-worker
  shim (e.g. the `coi-serviceworker` trick that re-serves with the headers) or by
  moving to a host that can set headers (Cloudflare Pages, Netlify, custom CDN).
- **The good news: the determinism core does not *need* threads.** `sim-core` is
  single-threaded by design (`roadmap.md`: "no Bevy dependency", deterministic
  ordering). Rollback determinism actually *wants* single-threaded execution. So
  the **first** web milestone can be **single-threaded wasm** — no
  SharedArrayBuffer, no COOP/COEP, deployable straight to gh-pages. Threads
  become relevant only if Bevy's renderer or asset pipeline wants them, which is a
  step-3 rendering question, not a sim question.
- **Message-passing vs shared memory.** Between main and worker, prefer
  `postMessage` with **transferable** objects (ArrayBuffers transferred, not
  copied) for the render-view and input. SharedArrayBuffer ring buffers (zero-copy
  input handoff) are an optimization that re-incurs the COOP/COEP tax — defer
  until measured need (the "snappy machine / don't add proactively" principle
  applies).

### Bevy in a worker — flag for just-in-time research

Bevy's app/schedule can run in wasm; the `GgrsSchedule` (step 5) gives exactly the
fixed-rate, locked-order loop a worker wants (breakdown step 5.1). But
**Bevy-in-Web-Worker + OffscreenCanvas + (optionally) wasm-threads is a fast-moving
area** across Bevy / wasm-bindgen / wgpu / winit. Treat the split above as
provisional and **do fresh deep-research + context7 at step 3** (rendering bring-up)
and **again at step 5** (rollback). The durable decisions — *sim in a worker,
inputs in, render-view out, single-threaded first* — are safe to assume now; the
exact API mechanics are not.

---

## 3. WebRTC peer-to-peer — translating the ENet/libjuice/Go stack

This is where the existing stack maps almost one-to-one. The table is the heart of
the doc.

| C++ today | Web equivalent | Notes |
|---|---|---|
| **ENet** reliable-ordered channel | **RTCDataChannel** with `ordered: true` (reliable, default) | Carries handshake, player info, match settings, map data, TC bundle, checksums — the `kPacket*` control messages in `transport.hpp:38-64`. |
| **ENet** unreliable-sequenced (input batches) | **RTCDataChannel** with `ordered: false, maxRetransmits: 0` | Carries `kPacketInputBatch` (`transport.hpp:59`). Loss-tolerant by design: the K-wide redundant window re-sends recent inputs every tick, so dropping a datagram is harmless — exactly why ENet used unreliable-sequenced here. |
| **libjuice** `IceAgent` (ICE/STUN/TURN) | **RTCPeerConnection** (browser's built-in ICE agent) | The browser *is* the ICE agent. STUN/TURN servers are passed in `RTCConfiguration.iceServers`; the browser does gathering, connectivity checks, and selection. Replaces `iceAgent.{hpp,cpp}` and the entire `IceBridge` loopback-socket hack (`iceBridge.hpp`) — that hack only existed to let unmodified ENet ride libjuice; WebRTC needs no bridge. |
| **IceBridge** loopback UDP socket pair | *(deleted)* | `iceBridge.hpp`'s job — feed ENet a socket while libjuice does the real I/O — vanishes; DataChannels are the socket. |
| **`SignalingClient`** (ENet raw UDP) + **Go server** | **WebSocket signaling client** + the **same Go server, reshaped to WebSocket** | The Go server's *logic* is already exactly right (pair by room code, forward credentials/candidates to the other peer — `server.go` `handleIceCredentials`/`handleIceCandidate`/`handleIceGatherDone`). Only the *transport* changes: browsers can't send raw UDP, so signaling moves to WebSocket, and the forwarded payload becomes the SDP offer/answer + ICE candidate JSON that `RTCPeerConnection` produces. Room codes (`generateRoomCode`, `server.go:340`) and matchmaking stay. |
| **STUN** (`stun.l.google.com:19302`, `iceAgent.hpp:21`) | same public STUN, via `iceServers` | NAT discovery; free. |
| **TURN** credential issuance (HMAC, `generateTurnCredentials`, `server.go:308`) | same, handed to the browser in `iceServers` | The Go server already mints time-limited TURN creds (coturn REST-API style). That code is reusable verbatim; only the delivery path (WebSocket message instead of UDP `sendRoomResponse`) changes. |
| **`tcArchive`** pack/unpack (FNV-1a hash + miniz, `tcArchive.hpp`) + **`memoryFs`** mount (`memoryFs.hpp`) | TC bundle over a reliable DataChannel **or** fetched from the CDN | Two options (§4): stream the compressed TC blob peer-to-peer over the reliable channel (`SendTcData`, `transport.hpp:155`), exactly as today, and mount it in an in-memory FS in the worker; **or** have both peers fetch the named TC from the CDN by hash and skip the transfer. The hash handshake (`SendTcInfo`/`SendTcResponse`, `transport.hpp:153-154`) decides whether transfer is even needed — keep it. |
| **`NetSession`** state machine (`session.hpp:21-29`) | unchanged in shape | connect → handshake → play → rematch/disconnect. The lifecycle, handshake (seed + settings hash, `transport.hpp:143`), and desync ring buffer (`session.hpp:180`) all port directly; only the transport object underneath swaps from `NetTransport` to a WebRTC-backed equivalent. |

### The handshake flow, web edition

1. Peer A opens the menu, connects to the **signaling WebSocket**, creates a room,
   gets a room code (today: `MsgCreateRoom` → `MsgRoomCreated`, `protocol.go:33,44`).
2. Peer B enters the code, joins; the server pairs them and notifies both
   (`MsgPeerJoined`).
3. Each peer creates an `RTCPeerConnection`, generates an **SDP offer/answer** and
   **ICE candidates**, and sends them through the signaling socket; the server
   forwards each to the other peer (the role currently played by
   `handleIceCredentials`/`handleIceCandidate` — `server.go:167,225`).
4. The browsers' ICE agents do STUN (and TURN if needed), pick a candidate pair,
   and the **DataChannels open**. Signaling is now done; the server can forget the
   room.
5. The `NetSession`-equivalent runs its handshake over the reliable channel (seed,
   settings, map/TC sync), then enters Playing and the rollback loop streams input
   batches over the unreliable channel — identical to today from this point on.

### Why DataChannel semantics line up so well

The existing transport already split traffic into "reliable control" and
"unreliable redundant input" (the two channel personalities in `transport.hpp`).
WebRTC DataChannels are configured per-channel with exactly those two
personalities. The redundant K-wide input window (`rollbackController.hpp:20`) was
designed for a lossy unreliable channel; an unordered/unreliable DataChannel is
its natural home. **No protocol redesign is needed — only a transport swap.**

---

## 4. CDN / hosting and the thin backend

### What's static (the bulk)

The wasm module, the JS glue, and the assets (`data/`, TC bundles) are **static
files**. They go on a CDN / static host. This is already proven on the C++ side:
the emscripten build produces `openliero.html/.js/.wasm/.data` and CI
**auto-deploys to gh-pages** (interactive-iteration doc §1a:
`.github/workflows/pages.yml`). The Rust/Bevy wasm build is the same kind of
artifact and slots into the same deploy. Benefits: zero compute cost, global
edge caching, trivially scalable, and the user's "open a URL on any device" goal.

**The COOP/COEP caveat (from §2):** if/when the build needs wasm-threads, the host
must send the cross-origin-isolation headers — which **gh-pages cannot**. The
practical ladder:

1. **Single-threaded wasm → gh-pages works as-is.** (Recommended first milestone.)
2. **Need threads but want to stay on gh-pages → `coi-serviceworker` shim**
   (a service worker that re-adds COOP/COEP). Works, slightly hacky, costs a
   first-load service-worker registration.
3. **Need threads cleanly → move to a header-capable static host** (Cloudflare
   Pages, Netlify, S3+CloudFront with a response-headers policy). Still static,
   still cheap, just not gh-pages.

### What's dynamic (the thin part)

The **only always-on server is signaling**, and it is genuinely tiny:

- It forwards a handful of small messages (room create/join, SDP, ICE candidates)
  per match *at connect time*, then is idle (`roomTTL` cleanup, `server.go:18`).
- It holds **no game state and sees no gameplay traffic** — zero per-frame load.
- It is nearly stateless: a map of room code → two peer handles
  (`Server.rooms`, `server.go:37-42`), GC'd on TTL.
- The existing Go server is ~250 lines and already does this; the web version
  reshapes it to WebSocket but keeps the logic. It can run on the cheapest
  always-on tier (a single small instance, or even a serverless/edge WebSocket
  service) and serve many concurrent rooms.

The **TURN relay is the one place a real cloud cost can reappear** — and only when
direct P2P connectivity fails (symmetric NATs, restrictive firewalls). TURN
relays the actual media/data, so it consumes bandwidth proportional to relayed
matches. Mitigations the engine already supports:
- TURN is **fallback-only**; most peers connect directly via STUN (free).
- The Go server already issues **time-limited HMAC TURN credentials**
  (`generateTurnCredentials`, `server.go:308`) and only when `TURN_SECRET` is set
  (`server.go:48`) — so TURN is opt-in and credential-gated, not always-on.
- A self-hosted `coturn` or a metered TURN provider can be added later; it is not
  needed for the first P2P milestone (LAN / good-NAT testing first).

**"Lightweight backend" concretely means:** static files on a CDN + one small
signaling process + optional TURN. No game servers, no databases of game state,
no per-frame server bandwidth, cost that scales with *connection setups*, not with
*gameplay*.

---

## 5. Roadmap fit and the determinism dividend

This work lives at the intersection of **step 3** (rendering, "playable image in a
window **and** in the browser (Wasm)") and **step 5** (bevy_ggrs rollback
netplay). It is explicitly forward-looking; nothing here should complicate steps
0–2.

- **Step 3 gives the wasm bring-up.** The breakdown's step-3 sub-slice 5 ("Wasm
  target bring-up… may surface asset-loading and threading constraints") and its
  "Wasm gotchas" note (no threads by default, async asset loading, indexed-palette
  rendering) are exactly the §2/§4 constraints. The Web Worker + single-threaded
  + gh-pages milestone is the concrete first cut of that sub-slice.
- **Step 5 gives the rollback.** GGRS provides the rollback schedule
  (`GgrsSchedule`, breakdown step 5.1) and the snapshot/restore + frame checksum
  (step 5.2). Crucially, **GGRS is transport-agnostic** — its session is fed by a
  pluggable socket — so a WebRTC DataChannel can back it directly. The redundant
  input window and generation tagging GGRS uses are conceptually the same as the
  C++ `RollbackController`'s (`rollbackController.hpp` generations/batches), so the
  C++ `test_rollback_*` suite remains the menu of scenarios to port (breakdown
  step 5).
- **Assess `matchbox` / `bevy_matchbox`.** This is the ecosystem-standard "WebRTC
  for GGRS in the browser" path: `bevy_matchbox` provides WebRTC DataChannel
  sockets (reliable + unreliable, matching our two channel personalities) that
  plug straight into bevy_ggrs, and `matchbox_server` is a small Rust signaling
  server. It would replace **both** libjuice **and** the Go signaling server with
  one ecosystem-blessed component — a genuinely attractive simplification, and the
  shortest path to a working P2P web build. The decision (research at step 5):
  - **Adopt matchbox** → least custom code, idiomatic bevy_ggrs, but a new Rust
    signaling server (`matchbox_server`) instead of the existing Go one, and its
    own message format/matchmaking model to learn.
  - **Reuse the Go server** → keep proven signaling + TURN-credential code, write
    a thin WebRTC DataChannel socket that speaks GGRS's socket trait. More custom
    glue, but preserves the existing `server/` investment and its TURN handling.
  - A hybrid (matchbox client socket, Go server reshaped to matchbox's signaling
    protocol) is also worth weighing. This is a real open question (below).
- **The determinism dividend is the same oracle, three times over.** The
  per-frame checksum (`HashGameState`, the step-2 oracle; `SendChecksum` on the
  wire; `NetSession` desync ring) is simultaneously: the differential-test oracle
  (step 2), the rollback desync detector (step 5, exactly as the C++
  `NetSession::OnChecksum` does, `session.hpp:176`), **and** the cross-platform
  guarantee that wasm and native agree. One mechanism, and it is already built in
  C++. The web version doesn't add a new correctness mechanism — it reuses the
  crown jewel.

---

## 6. Realistic assessment — honest trade-offs

- **Determinism must survive the wasm target (the crown-jewel risk).** The whole
  thesis rests on wasm and native producing **bit-identical checksums**. The
  engine's integer/fixed-point discipline (`sim-core`, no floats) is exactly what
  makes this plausible: wasm's `i32`/`i64` arithmetic is well-defined and
  deterministic, unlike float. But this **must be proven**, not assumed — the
  step-2 time-series oracle should be run with the **wasm build as a target** and
  its checksums diffed against native, early in step 3. If any float leaked into
  the sim (the breakdown's recurring warning), wasm is where it bites. Verdict:
  high-confidence given the architecture, but a must-test, not a given.
- **COOP/COEP vs gh-pages (hosting friction).** Covered in §4: single-threaded
  first sidesteps it entirely; threads force a header-capable host or a
  service-worker shim. The risk is doing threads-first and discovering the gh-pages
  deploy (the proven path) no longer works. Mitigation: stay single-threaded until
  a measured need forces threads.
- **wasm bundle size.** Bevy + wgpu + the sim is a non-trivial download.
  Mitigations exist (wasm-opt, code stripping, gzip/brotli on the CDN, lazy asset
  loading) but it is heavier than the C++ emscripten build. First-load time on the
  share link is the user-visible cost. Defer optimization until there's a working
  build to measure.
- **WebRTC connection reliability + TURN cost.** Direct P2P fails for some
  NAT/firewall combinations; without TURN those peers simply can't connect. TURN
  fixes it but costs relayed bandwidth. The engine already treats TURN as
  credential-gated fallback (§4), so this is a cost *toggle*, not a baseline cost —
  but "some players need TURN" is a real operational fact to plan for before a
  public launch.
- **Bevy-in-worker + OffscreenCanvas maturity.** The cleanest architecture
  (sim+render in the worker) leans on fast-moving Bevy/wgpu/winit wasm support.
  Risk it isn't smooth at step 3. Fallback: sim-in-worker + render-on-main-thread
  (more marshalling, less bleeding-edge). Re-research at step 3; don't lock the
  split now.
- **Debugging in the browser is harder.** As the interactive-iteration doc notes,
  the hard 10% (determinism) is best debugged natively. A wasm checksum divergence
  should be reproduced and chased on native first; the browser is for confirming
  parity, not for primary determinism debugging.

### Milestone ladder

- **M0 — single-player wasm in a Web Worker on gh-pages.** Build the step-3 wasm
  renderer, move the sim into a worker, single-threaded, no COOP/COEP, deploy to
  gh-pages (reuse the C++ deploy precedent). **Gate:** the wasm build's per-tick
  `HashGameState` matches native on the step-2 oracle scenarios (proves the crown
  jewel survived wasm). This is the lightweight-web foundation and a shareable URL.
- **M1 — two-peer P2P over WebRTC.** After step 5's local two-session rollback
  works, back GGRS with a WebRTC DataChannel (matchbox or custom-over-Go), reshape
  the signaling server to WebSocket, exchange input batches peer-to-peer. **Gate:**
  the Rust equivalent of the C++ `test_rollback_*` suite passes across two browsers
  (checksums agree under jitter/loss/reorder), STUN-only on good NATs.
- **M2 (later) — robustness + reach.** TURN fallback for hard NATs, bundle-size
  optimization, optional wasm-threads (with the header-capable host), matchmaking
  niceties. None of this is on the critical path to "it works P2P in a browser."

---

## Open questions for the controller

- **matchbox vs. reuse-the-Go-server for WebRTC + signaling** (§5). The biggest
  fork: adopt `bevy_matchbox` + `matchbox_server` (least code, idiomatic ggrs, new
  signaling server) vs. a custom WebRTC DataChannel socket behind GGRS feeding off
  the existing Go server reshaped to WebSocket (preserves proven signaling + TURN
  code). Decide at step 5 with fresh research.
- **Threads or not, and therefore which host.** Commit to single-threaded wasm on
  gh-pages for M0 (recommended), or invest early in a COOP/COEP-capable host to
  keep wasm-threads open? Affects the deploy story from day one.
- **Sim-render split in the worker:** OffscreenCanvas (render in worker) vs.
  render-on-main-thread with a per-tick render-view message? Depends on
  step-3-era Bevy/wgpu wasm maturity — research then.
- **TC delivery on the web:** stream the bundle peer-to-peer over the reliable
  DataChannel (reuse `tcArchive`/`memoryFs` as-is) vs. fetch named TCs from the
  CDN by hash? The hash handshake (`SendTcInfo`/`SendTcResponse`) lets us support
  both — which is the default?
- **How early to prove wasm determinism.** Run the step-2 oracle against the wasm
  build as part of M0's gate (recommended), or treat it as a step-5 concern? (This
  doc argues: prove it at M0 — it's the whole thesis.)
- **Does the signaling server stay Go** (reshaped to WebSocket) **or become Rust**
  (matchbox_server / a small axum service) to keep the web stack single-language?

---

*Exploration only. No code written, no commits; `rust/` and `src/` untouched.*
