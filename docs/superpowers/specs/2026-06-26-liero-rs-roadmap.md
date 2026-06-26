# Liero-rs — roadmap för en agent-driven Rust/Bevy-omskrivning

Status: **utkast för granskning** · 2026-06-26

En strategi för att skriva om OpenLiero-simuleringen i Rust + Bevy, inkrementellt,
med AI-agenter, i **samma repo** (monorepo) som C++-motorn. Detta dokument är
roadmap-altituden ("allt"); varje steg får ett eget detaljspec just-in-time.

## Mål och drivkrafter

- **Lära sig** modern Rust/Bevy/ECS och agent-driven utveckling (huvudpoäng).
- **Räckvidd**: web (Wasm) och på sikt mobil.
- **Nya förmågor**: utrymme för större banor, fler spelare, moddbarhet.
- **Underhållbarhet**: tydliga moduler, hårt testat.
- Tid är *ingen* begränsning. Det får vara svårt och ta tid.

## Bärande princip: determinismen är kronjuvelen

Lieros själ är **deterministisk fixpunkts-simulering + rollback-netcode**. 90 % av
en omskrivning är lätt (sprites, menyer, ljud). De sista 10 % — att tick nr 1000
blir *bit-exakt* lika på två maskiner — är det svåra, och det som gör replay och
rollback-nätspel möjligt. En omskrivning som tappar determinismen tappar spelet.

Därför: **ingen big-bang-omskrivning.** Strangler-mönster — bygg den nya motorn
bit för bit, bevisa varje bit korrekt mot den gamla, behåll den gamla tills den
nya är klar.

## Orakel-strategin: differentialtestning mot C++

Den befintliga C++-motorn behålls som **sanningsorakel**. För varje delsystem i
den nya motorn matas exakt samma input genom båda, och tillståndet jämförs
checksum-mässigt tick för tick. Matchar de bit-för-bit är den nya delen *bevisat*
korrekt; annars pekar diffen ut exakt vilken tick som divergerar.

Detta är vad som gör agent-arbetet säkert: en stor, skrämmande omskrivning blir en
lång rad **små, oberoende, objektivt verifierbara uppgifter** — den form agenter
är bäst på. Monorepo gör det enkelt: båda kodbaserna ligger sida vid sida och CI
kan köra "ny motor matchar gammal motor" som ett vanligt test.

## Teknikval

| Lager | Val | Varför |
|---|---|---|
| Språk | **Rust** | Heltalsmatematik, ingen GC → naturlig determinism; Wasm-mål; typsystemet gör agent-kod säkrare |
| Engine | **Bevy** (ECS) | Modern, kod-först, ECS passar entitets-tunga spel; batterier inkluderade |
| Rollback | **bevy_ggrs** (GGRS) | Moget rollback-ekosystem; checksum per frame återanvänds som orakel |
| RNG | **bevy_rand** + portad MT19937 | RNG måste vara del av rollback-tillståndet, seedad och återställbar |
| Determinism-kärna | egen `sim-core`-crate, **utan Bevy-beroende** | Skyddar fixpunkt/RNG från Bevys API-churn; testas isolerat |

**Bevy-fällan att hantera:** Bevy kör system parallellt och i ej garanterad
ordning. Rollback kräver motsatsen. bevy_ggrs löser det via en `GgrsSchedule`
(fast takt, låst systemordning), explicit registrering av rollback-tillstånd, och
RNG som del av det tillståndet. Determinism-disciplin i sim-systemen är ett krav,
inte en bonus.

## Monorepo-layout

Rust-koden bor i en egen top-level-katalog, parallellt med `src/` (C++) och
`server/` (Go):

```
openliero/
├── src/            C++-motorn (oraklet, behålls)
├── server/         Go signaling/relay (behålls)
├── rust/           ← NYTT: cargo-workspace
│   ├── sim-core/     ren Rust, ingen Bevy: fixed, rng, vec, tables
│   ├── game/         Bevy-appen (ECS-komponenter, system, rendering)
│   └── oracle-tests/ differentialtester mot C++ (golden vectors)
└── data/           delade assets (samma data = samma orakel)
```

## Stegen (strangler-ordning)

Varje steg differentialtestas mot C++ innan nästa börjar.

| # | Steg | Klart när … |
|---|---|---|
| **0** | **Deterministisk grund** — `sim-core`: fixpunkt + RNG | Rust reproducerar C++:s `math` och `Rand` bit-exakt (golden vectors gröna) |
| 1 | **Asset-/dataformat** — läs banor, TC, sprites | Rust laddar samma `data/`-filer som C++ och får identiska bytes |
| 2 | **Sim-kärna i ECS** — Level → Worm → ett vapen → `processFrame` | En kula skjuts, rör sig, exploderar, förstör terräng — checksum matchar C++ tick för tick |
| 3 | **Rendering** — Bevy ritar världen | Spelbar bild i fönster och i webben (Wasm) |
| 4 | **Loop + input** — fast takt, tangentbord | Spelbart single-player, känns som Liero |
| 5 | **bevy_ggrs** — rollback-nätspel | Två klienter spelar samma match, desync-fri |

Steg 2–5 detaljspecas just-in-time; vi förstår dem bättre efter steg 0–1.

## Risker och hur oraklet avlastar dem

- **Subtil determinism-divergens** → fångas omedelbart av checksum-diffen; vi får
  exakt vilken tick och vilket fält som skiljer.
- **Bevy-API-churn** → determinism-kritiska delar isoleras i `sim-core` utan
  Bevy-beroende.
- **Agent producerar plausibel men felaktig kod** → kan inte passera oraklet;
  korrekthet är objektiv, inte en bedömningsfråga.
- **Scope-skav** → strangler + just-in-time-spec hindrar överplanering av delar vi
  ännu inte förstår.

## Medvetet uppskjutet (YAGNI tills vidare)

Mobil-paketering, nya spellägen, större banor, moddverktyg, 3D. Allt detta blir
möjligt *efter* att en deterministisk kärna finns — men inget av det får
komplicera steg 0–2.

## Nästa konkreta artefakt

Detaljspec för **steg 0**:
`2026-06-26-liero-rs-step0-deterministic-foundation-design.md`.
