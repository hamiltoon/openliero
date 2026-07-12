# Step 3, Slice 3e — HUD / font / bars / minimap (the full player view): design

Status: **spec** · 2026-07-12 · companion to `2026-07-10-liero-rs-step3-rendering-overview.md`
(cited as **overview**) and `2026-07-10-liero-rs-step3-cpp-render-pipeline-map.md` (cited as
**render-map §N**). Prior slice specs/plans: `slice3a-render-foundation-design.md`,
`slice3b-plan.md`. Lands on branch `liero-rs-step-3` (**PR #4**, accumulating).

This is the **last pixel-gated render slice** before wasm (3f). 3a–3d shipped the pixel-exact
world view (terrain → shadows → sprites), put it LIVE natively (`game`, 3c), and gave the agent a
headless screenshot/compare loop (`shot`, 3d). 3e completes the **full in-game player frame** —
the HUD bars, the kills/lives/reloading text (which needs a real **font**), and the 52×36
**minimap** — pixel-gated by the same FNV-1a frame-hash oracle. When 3e lands, the whole
`Viewport::Draw` surface (minus explicitly deferred game-mode chrome) matches C++ byte-for-byte,
and 3f only has to render that same frame in a browser.

---

## Goal / done-when

Port the **HUD half** of `Viewport::Draw` (`viewport.cpp:78-189` HUD pre-block + `:593-635`
minimap) into the Bevy-free `render` crate, plus the **font pipeline** (`font.tga` load +
`Font::DrawChar/DrawString`) and `DrawBar`, and prove the **full player-view frame hash** matches
C++ tick-for-tick over new HUD-including goldens.

**Done when:**
1. The full player-view frame hash (world block **+** HUD bars/text **+** minimap) matches the
   C++ dumper tick-for-tick over the new `render_slice3e_*` goldens (Classic mode).
2. Every reachable HUD element is proven **non-vacuous** (a control render where the element is
   suppressed produces a different hash — à la 3b's shadow/laser proofs).
3. Both **re-diff gates stay green**: every `sim_slice*.txt` byte-identical, AND every prior
   `render_slice3a.txt` / `render_slice3b_*.txt` byte-identical (the HUD is behind a new opt-in
   directive; absent it, the world-only draw is unchanged).
4. The isolation triple holds per tick (`state_hash` Rust == sidecar == pre-existing sim golden);
   the `render` crate stays Bevy-free (`cargo tree -p render` shows no `bevy*`).

**Non-goals (unchanged from overview):** wasm (3f); keyboard input/audio/live shake-flash wiring
(Step 4); follow-cam; render interpolation; Modern color mode; the spectator renderer; menus.

---

## 1. What is in C++'s full player view — the HUD map (with `file:line`)

`Viewport::Draw` (`viewport.cpp:78-636`) draws, in strict order, **HUD-pre-block (unclipped) →
[clipped world block] → minimap (unclipped)**. 3a–3b shipped the middle (clipped world block,
`:196-591`). 3e is the two bookends. The world-view section is already pixel-exact; 3e adds:

### HUD pre-block (before the world clip is set), `viewport.cpp:84-189`

| # | Element | C++ | Draws via | Reachable in our scenarios? |
|---|---------|-----|-----------|------------------------------|
| 1 | **Life bar** (visible worm) | `:84-87` | `DrawBar` | **YES** — worms visible; `health*100/maxhealth`, color `w/10+234` |
| 1b | **Life bar** (dying worm countdown) | `:88-95` | `DrawBar` | **YES iff a death scenario** — `100-(killed_timer*25)/37` |
| 2 | **Ammo bar** (weapon avail, ammo>0) | `:101-109` | `DrawBar` | **YES iff a weapon/ammo scenario** — `ammo*100/type.ammo`, color `w/10+245` |
| 2b | **Loading bar** (reloading) | `:110-124` | `DrawBar` | **YES iff a reload scenario** — `100 - loading_left*100/loadtime` |
| 3 | **"Reloading" text** | `:125-128` | `Font::DrawString` | **YES iff reloading** — gated `(cycles%20)>10 && visible`, `LS(Reloading)` color 50 |
| 4 | **Kills text** | `:131-132` | `Font::DrawString` | **ALWAYS** — `LS(Kills)+ToString(kills)`, color 10 |
| 5 | Replay: worm name / color box / match time | `:134-144` | `Font::DrawString`,`FillRect` | **NO** — `is_replay==false` (in-game view) → **defer w/ tripwire** |
| 6 | **Lives text** (KillEmAll/Scales) | `:148-153` | `Font::DrawString` | **YES** — `LS(Lives)+ToString(lives)`, color 6 |
| 6b | Holdazone/GameOfTag timer text | `:155-189` | `Font::DrawString` | **NO** — game_mode==KillEmAll; Holdazone deferred past Step 2 → **defer w/ tripwire** |

### Inside the world block (still 3e work, not covered by 3b)

| # | Element | C++ | Reachable? |
|---|---------|-----|-----------|
| 7 | Holdazone dashed box | `:212-233` (`DrawDashedLineBox`) | **NO** (Holdazone) → defer w/ tripwire |
| 8 | "Press fire" text (pre-spawn) | `:235-237` (`Font::DrawString`) | **only if a worm sits in pre-spawn** → defer unless a scenario reaches it |
| 8b | Spawn-preview worm sprite | `:239-246` (`BlitImageTrans`) | **NO** — `allow_viewing_spawn_point && Pressed(kChange)` (3b deferral) → defer w/ tripwire |
| 9 | Death banners (self/other) | `:249-270` (`Font::DrawString`) | **only in a death scenario** — `banner_y>-8 && health<=0` |
| 10 | Name labels (bonus/wobj/weapon-change) | `:408-413`,`:465-479`,`:575-582` (`DrawTextSmall`) | **NO** — `names_on_bonuses=false` / `Pressed(kChange)` (3b deferral) → defer w/ tripwire |

### Minimap (after the world clip is restored), `viewport.cpp:593-635`

| # | Element | C++ | Reachable? |
|---|---------|-----|-----------|
| 11 | **Minimap terrain** | `:598-602` (`Level::DrawMiniature`, `level.cpp:489`) | **YES iff `settings.map`** — 52×36 (`level.hpp:30-31`), steps the material grid through `AppearanceAt` |
| 12 | **Worm dots** | `:604-613` (`SetPixel`, `MinimapColor()=129+index*4`, `worm.hpp:203`) | **YES** (visible worms) |
| 13 | Holdazone minimap marker | `:615-634` (`SetPixel` ×8) | **NO** (Holdazone) → defer w/ tripwire |

**Layout constants (framehash player layout, render-map §7):** `render_res 320×200`, `kMultiplier
= 320/320 = 1`, `kCenterX = 160`. `stats_x` = **0** (worm 0) / **218** (worm 1) — already a
dumper scenario field (`worm <idx> .. <stats_x> <visible>`, `sim_physics_dump.cpp:46,251`). Life
bar y=`200-39=161`, ammo y=`166`, reloading y=`164`, kills y=`171`, lives y=`178`. Minimap
`kMapX = 160-26 = 134`, `kMapY = 200-38 = 162`, `kMinimapStepX/Y = max(ceil(dim/52|36),1)`.
**NB both viewports draw the minimap at the *same* centered position** (`kCenterX`-relative) — the
second overwrites the first; reproduce verbatim.

### Verdict — reachable NEW surface for KillEmAll / 2 worms / current scenarios

**Always/base-reachable:** life bar (visible), kills text, lives text (KillEmAll), minimap
terrain, worm dots. **Reachable with a firing/reload scenario:** ammo bar, loading bar, reloading
text. **Reachable only with a death scenario:** dying-worm life bar (1b), death banners (9).
**Everything else deferred with a tripwire** (`debug_assert!`/`unreachable!`), exactly as 3b
deferred `bonus_frames`/spawn-preview/name-labels: Holdazone (box, timer text, minimap marker —
Holdazone is deferred past Step 2, PROGRESS T5), GameOfTag timer, replay name/box/time,
spawn-preview, name-labels, `DrawTextSmall`, "Press fire" (unless a scenario reaches it), AI debug
(permanently out).

---

## 2. Font pipeline

C++ loads two glyph banks in `Common::load` (`common.cpp:365-435`):

- **`text.tga` → `text_sprites`** (4×4, 26 sprites, `common.cpp:370,400-403`) — plain
  `ReadSpriteTga` into a `SpriteSet`. Consumed **only** by `DrawTextSmall` (`common.cpp:227-237`),
  whose every call site is a **deferred** name-label. → **`text_sprites` + `DrawTextSmall` are
  DEFERRED** in 3e (unreached; loading + porting them would be gold-plating). Tripwire only.

- **`font.tga` → `Font`** (`common.cpp:405-434`) — **not** a plain `SpriteSet`. It is read as a
  raw 7-wide × `250*8`-tall buffer, then post-processed per char into `Font::Char { data[8*7],
  int width }` (`font.hpp:11-14`): for each of the 250 chars, scan its 7×8 cell row-major; pixel
  `p==0 → 0`, `p==50 → 8`, **else** set `width = x` and `break` the row (a sentinel column marks
  the glyph's advance width). So glyph "on" pixels are stored as palette index **8**, and per-char
  `width` is auto-detected. This must be ported **bit-exactly** (the pixel gate hashes it), but it
  is **not sim-affecting** (pure asset transform). Font is Classic/render-only.

**`Font::DrawChar`** (`font.cpp:8-43`): index-0-transparent 7×8 blit, writes `pal32[color]`
(color is the caller's arg, not the glyph value); `size` scales; `CLIP_IMAGE` clips. **Guard**:
`if (c >= 2 && c < 252)` on the **already-decremented** `c` (the "TODO is this correct" — port the
quirk verbatim). **`Font::DrawString`** (`font.cpp:58-80`): UTF-8 → codepoint (`Utf8DecodeNext`) →
CP437 byte (`UnicodeToByte`), skip if `<2 || >=252`, then `c -= 2`, `DrawChar(c)`, advance `x +=
chars[c].width * size`. Newline (cp==0) resets x and `y += 8*size`.

**CP437 decode (`cp437.cpp`):** the reachable HUD strings in the shipped openliero TC are **pure
ASCII** — verified: `Reloading = "Reloading..."`, `Kills = "Kills: "`, `Lives = "Lives: "`
(`data/TC/openliero/tc.cfg`), plus `ToString(int)` digits. For printable ASCII, UTF-8 decode is
identity and the CP437 byte == the ASCII byte, so a **minimal ASCII decode path is sufficient and
bit-exact** for 3e. **Recommendation:** port an ASCII-only decode with a **proof-test** that
`Kills`/`Lives`/`Reloading` (+ digits) are all `< 0x80`; defer the full CP437 table (only a
non-ASCII TC string — e.g. `Copyright2 = ä` — would need it, and no such string is HUD-reachable).
`Texts { Reloading, Kills, Lives }` **already parses** in `assets::tc` (`tc.rs:239-242`) — thread
those verbatim (no new asset parsing for the label strings).

**Where Font lives:** a new `render::font` module (`Font` struct + `Font::load(font_tga_bytes)`),
since the glyph transform is render-only; the `font.tga` **bytes** are read in `scenario::load`
(alongside `small.tga`/`large.tga`) and handed to `Font::load`, producing a `Font` in the loaded
`Scene`. `ToString(int)` = Rust `i32::to_string` (decimal, matches C++).

---

## 3. Dumper verdict — the C++ render path does NOT draw the HUD (must be extended)

**Verdict: the dumper's `render_and_hash` renders ONLY the world block** (`sim_physics_dump.cpp:69-74,
453-805`) — the palette build → `Fill(0)` → per-viewport `Process` → clip → `DrawLevel` → shadow
pass → sprite pass → hash. **HUD bars/text, minimap, name-labels, banners, holdazone, AI-debug are
explicitly OMITTED** (comment `:74`, `:643-644`). The dumper builds bare `Viewport`s and inlines
the world subset; it never calls `Viewport::Draw` and never touches `settings->map` or
`is_replay`. **So the dumper MUST be extended** to also draw the HUD pre-block + minimap for 3e —
this is a **C++ change** (as 3b extended it for the shadow/sprite passes).

**Re-diff discipline (mandatory, T6-lesson):** the HUD draw MUST be behind a **new opt-in scenario
directive `render_hud`** (a 0-arg presence flag, mirroring `render_shadow`). When **absent** (every
existing 3a/3b render scenario), `render_and_hash` runs exactly as today → `render_slice3a.txt` and
all `render_slice3b_*.txt` regenerate **byte-identical** (a done-when assertion, not a regen: any
stray pixel is a real bug). When **present**, the dumper *also* draws:
- the HUD pre-block **into the full-surface clip, before the per-viewport world clip is set** (bars
  at `worm.stats_x`, text via `Font::DrawString`), for each viewport's worm;
- the minimap **after** the world block, into the full-surface clip, gated on `settings->map`
  (which the scenario sets — see below), for each viewport (same centered position).

The directive parser arm must be added on **both sides in the same commit** (Rust
`oracle-tests`/`scenario` parser **and** C++ `ParseScenario` — the T6 lesson from 3a/3b: new
scenario tokens require both parsers). `render_hud` sets `settings->map = true` and `is_replay =
false` for the draw only (sim-neutral: the flip happens after the tick's sim `Process`, is
restored before the next, and neither `map` nor `is_replay` reaches a sim mutation — same
discipline as `render_shadow`). `stats_x` and `game_mode` are **already** scenario fields, so no
new sim-side plumbing is needed for HUD positioning.

**Consequence for goldens:** because the HUD writes into the same 320×200 frame, the existing
world-only goldens **cannot** carry HUD pixels — 3e produces **new** `render_slice3e_*` goldens
(with `render_hud`), and the world-only 3a/3b goldens stay frozen as the re-diff proof.

---

## 4. Golden strategy — minimal non-vacuous set

New `render_slice3e_*` scenarios reuse existing Step-2 **sim input vectors** + `render` +
`render_hud` (+ `render_shadow` where useful). Minimal set to make every reachable HUD element
non-vacuous:

1. **`render_slice3e_hud`** (base — reuse the `blood` inputs, visible worms, KillEmAll,
   `render_hud`, `settings.map`, `stats_x` 0/218): proves **life bar (visible)** + **kills text** +
   **lives text** + **minimap terrain** + **worm dots**. *Non-vacuity controls:* re-render the peak
   tick with (a) `map=false` → minimap+dots gone (hash moves); (b) HUD off (`render_hud` absent =
   the frozen 3b `blood` hash) → HUD gone (hash moves); (c) a worm-dot position shift across ticks.

2. **`render_slice3e_reload`** (reuse a firing scenario — `dart`/`fan`/handgun-like — a worm fires
   and reloads): proves **ammo bar** + **loading bar** + **"Reloading" text**. *Non-vacuity:* the
   ammo/loading bar **width changes across ticks** as `ammo`/`loading_left` step (assert distinct
   per-tick hashes in the reload window), and the reloading text blinks on the `(cycles%20)>10`
   gate.

3. **`render_slice3e_death`** (OPTIONAL — reuse the 5d death→respawn window): proves the
   **dying-worm life-bar countdown (1b)** + **death banner (9)**. *Non-vacuity:* the banner text
   appears only while `banner_y>-8 && health<=0`. **Scope call (open Q3):** if T-count runs hot,
   defer 1b + banners (with tripwire) and land 3e on scenarios 1+2; the death scenario is the
   cleanest banner witness but banners are game-chrome, not core HUD.

Each `render_slice3e_*` ships a `_sim.txt` (normal 11-column sim golden, the isolation column) and
a `_scenario.txt`, generated locally via a `gen_render_slice3e_*.sh` (mirroring the 3b gen
scripts). No existing golden is regenerated — the gen scripts only write the new files.

---

## 5. Scope cut → task overview (~9 tasks; 3b was 10, 3d was 5)

- **T0** — `render::font::Font` + `font.tga` loader (7×8, 0/50→0/8, width-detect) + thread it +
  the `Texts` labels through `scenario::load`/`Scene`. RED: load the real `font.tga`, assert a
  known glyph's `width` + on-pixels==8.
- **T1** — `Font::DrawChar` + `Font::DrawString` + ASCII decode (+ ASCII proof-test on the reachable
  labels). RED: draw "Kills: 0", assert pixels + advance + `CLIP_IMAGE` clip + the `c-=2`/double-guard.
- **T2** — `DrawBar` (+ `FillRect` if a reachable use needs it; replay `FillRect` stays deferred).
  RED: bar width/height/color/clip.
- **T3** — HUD bar/text block (`render::hud`): life bar (visible + dying arms), ammo/loading bar,
  reloading text, kills text, lives text (KillEmAll/Scales arms only; Holdazone/GameOfTag/replay
  tripwired). RED: synthetic worm at `stats_x`.
- **T4** — Minimap: `Level::DrawMiniature` port (`level.cpp:489`) + worm dots (`MinimapColor`) +
  `kMapX/kMapY`/step math (Holdazone marker tripwired). RED: synthetic level minimap.
- **T5** — wire into `frame::draw`: extend `Scene` (font ref, label strings, `draw_hud`, `map`);
  `draw` calls HUD-pre-block per viewport → world block → minimap, gated on `draw_hud`. Update 3b
  harness/callers so world-only frame hashes stay **byte-identical** when `draw_hud=false`.
- **T6** — C++ dumper: add `render_hud` directive (both parser arms move together); draw HUD
  pre-block + minimap; set `settings->map`/`is_replay=false` for the draw only; **re-diff gate**
  (3a/3b + sim goldens byte-identical).
- **T7** — new `render_slice3e_*` scenarios + `_sim`/`_scenario`/frame goldens + gen scripts
  (base + reload; death optional).
- **T8** — Rust frame-hash golden tests (`render_slice3e_common` harness) + non-vacuity controls
  (map on/off, HUD on/off vs frozen 3b hash, ammo-bar per-tick movement) — **MILESTONE**.
- **T9** — docs (PROGRESS + overview slice-3e line + deferral ledger) + broad slice review.

---

## 6. Deferrals carried out of 3e (each with a tripwire)

- **`DrawTextSmall` + `text_sprites`** (name-labels — `names_on_bonuses=false`, no `kChange`).
- **Holdazone HUD**: dashed box (`:229`), timer text (`:166-184`), minimap marker (`:615-634`) —
  Holdazone is deferred past Step 2 (PROGRESS T5); no scenario sets `game_mode==Holdazone`.
- **GameOfTag** timer text + "YoureIt" banner (`:171-185,:250-253`).
- **Replay HUD**: worm name / color box (`FillRect`) / match time (`:134-144`) — `is_replay=false`.
- **Spawn-preview** `BlitImageTrans` (`:243`) + (unless a scenario reaches it) "Press fire" text.
- **Full CP437 table** (only non-ASCII TC strings need it; none HUD-reachable).
- **AI debug** (`:550`) — permanently out. **Modern**, **spectator**, **menus** — per overview.
- **`bonus_frames`** stays as 3b left it unless a bonus scenario enters 3e.

A future scenario reaching any of these lifts the deferral with a matching golden.

---

## 7. Open questions (with recommendations)

1. **`is_replay` for the in-game view — false.** *Recommendation: `false`.* The "full in-game
   player frame" (overview done-when) is the non-replay view; the replay-only name/color-box/time
   block (`:134-144`) is deferred. (If a later slice wants the replay HUD, flip it behind a
   `render_replay_hud` directive.)
2. **CP437 depth — ASCII subset now.** *Recommendation: ASCII-only decode + a proof-test that the
   reachable labels are `<0x80`; defer the full table.* Verified the shipped TC labels are ASCII.
3. **Death scenario (banners + dying life bar) — include or defer.** *Recommendation: include
   `render_slice3e_death` if the T-count allows (it is the cleanest banner/countdown witness);
   otherwise defer 1b + banners with a tripwire and land on hud+reload.* Banners are game-chrome,
   not core HUD, so deferring them does not weaken the "full player view" claim for the KillEmAll
   frame — but including them closes the last font-reachable path.
4. **Font location — `render::font`.** *Recommendation: `Font` struct + loader in the `render`
   crate (render-only transform); `font.tga` bytes read in `scenario::load`.* Keeps `assets`'s
   `SpriteSet` generic and the glyph post-processing beside its only consumer.
5. **`DrawBar`/`FillRect` scope.** *Recommendation: port `DrawBar` (reachable); port `FillRect`
   only if a reachable element needs it (`DrawBar` alone covers all reachable bars; `FillRect`'s
   only reachable-adjacent use is the deferred replay color box) — otherwise defer `FillRect`.*
6. **Minimap double-draw.** *Recommendation: reproduce verbatim* — both viewports draw the minimap
   at the same `kCenterX`-relative position; the second overwrites the first. Not a bug; it is the
   C++ frame, and the hash must match it.
