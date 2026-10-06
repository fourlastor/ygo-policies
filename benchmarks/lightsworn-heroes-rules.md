# Lightsworn Judgment and Fusion Heroes

Lightsworn Judgment (#17, 56.1%) and Fusion Heroes (#13, 60.0%) were the two
weakest remaining policies in the tuning queue. Baseline: `26d9e3dcde24cb18845201e350ad7f96ad8c901c`,
after Rock Block and Tele-DAD. This batch changes only these policies and
their tests on `codex/lightsworn-heroes-policies`. Decks, shared policy code,
and vendor pointers are unchanged.

## Held-out validation

After rule selection, each version played 512 games against every other deck,
using fresh seed `40350000`: 15,360 paired games per policy. Both versions face
the same baseline opponents, shuffles, seats, and policy randomness. Draws
count as half a win. No tuning used these held-out results.

| Pilot | Games per version | Before | After | Gain, pp [paired 95% CI] |
|---|---:|---:|---:|---:|
| Lightsworn Judgment | 15,360 | 56.19% | 63.95% | +7.75 [+7.09, +8.41] |
| Fusion Heroes | 15,360 | 59.40% | 68.15% | +8.75 [+8.09, +9.41] |

Lightsworn Judgment improved against 30 opponents, tied 0, and declined against 0. Fusion Heroes improved against 28 opponents, tied 0, and declined against 2. Individual matchup samples are smaller than the aggregate; see the summary for their results.

Negative matchup point estimates (512 pairs each), reported without further tuning:

- Fusion Heroes against countdown: -0.39 pp [-1.33, +0.55].
- Fusion Heroes against watt: -4.88 pp [-7.89, -1.87].

Heroes against Watt fell from **91.60% to 86.72%**. Its interval excludes
zero, so this is a material matchup tradeoff despite the overall improvement.
These held-out results were retained without further tuning.

Confidence intervals are paired normal approximations. No games failed or
reached the decision limit.

## Rules retained

Lightsworn:

- Prefer Wulf as a revival target, while preserving its low discard value.
  Its 2,100 ATK is useful on the field even though it is difficult to play
  from hand. Broader context-dependent Lumina/Lyla revival priorities were
  less successful than this focused change.
- Revive Plaguespreader when a hand card can pay its cost and a face-up
  non-Tuner can make a Synchro available in our Extra Deck.
- Summon Judgment Dragon before adding more millers or spending the Normal
  Summon. Its deck-out checks still apply.
- Allow Judgment Dragon's Special Summon with an estimated two turns of Deck
  remaining instead of three. The existing lethal and urgent-wipe exceptions remain.
- Use Judgment Dragon's wipe when the opposing field is worth at least
  1,800 and exceeds the sacrificed allies/backrow by more than 500, instead
  of the old 2,500/1,000 thresholds. Keep the existing Life Point gate.
- Flip Ryko proactively when the opponent has a removal target and the Deck
  budget permits another three-card mill. Recognize alternate passcodes.
- Use Compulsory Evacuation Device against a reachable face-up Extra Deck
  monster before waiting for an attack. Existing battle protection remains.

Fusion Heroes:

- Normal Summon Stratos before spending it as Fusion material, taking its
  summon trigger before committing the Fusion spell.
- Give Absolute Zero more weight when choosing a Fusion, accounting for its
  threat to destroy opposing monsters when it leaves the field.
- Stop ordinary Fusion sequences while an existing face-up Fusion and our
  strongest face-up monster already exceed the opposing best attack. This
  avoids repeatedly spending a winning board as material. A stronger opposing
  monster still permits further Fusions; Super Polymerization removal remains
  available independently.

All rules use permitted observations, public card data, and legal choices.
The policies have no access to opposing hidden identities, deck order, engine
snapshots, or future random outcomes.

## Development measurements

Each screen used seed `39240000`, 256 games per reference opponent: 3,072
pairs per policy. Both belong to the 13-deck reference pool and exclude their
own self-match. Screening gains are exploratory, not independent confirmation.

| Screen | Lightsworn candidate | Gain | Heroes candidate | Gain |
|---|---|---:|---|---:|
| 1 | Prefer Wulf for revival | +0.81 pp | Summon Stratos before Fusion | +1.20 pp |
| 2 | Revive Plaguespreader for Synchros | +2.96 pp | Ocean only with GY recovery | +0.13 pp |
| 3 | Judgment Dragon before setup | +1.79 pp | Higher Absolute Zero priority | +1.60 pp |
| 4 | Reduce general mill reserve | +0.39 pp | Preserve a winning Fusion | +4.62 pp |
| 5 | Proactive Ryko flips | +1.37 pp | Birdman summon hook (no effect) | +0.00 pp |
| 6 | Normal Tuners for Synchros | +0.85 pp | Broader King of the Swamp searches | +0.62 pp |
| 7 | Contextual Lumina/Lyla revival | +0.52 pp | Stratos searches over one backrow | -0.26 pp |
| 8 | Celestia tributes a Lightsworn | +0.36 pp | Proactive Snowman Eater flips | +0.00 pp |
| 9 | Compulsory on Extra Deck monsters | +0.55 pp | Preserve even an outmatched Absolute Zero | +4.43 pp |
| 10 | Unrestricted Glorious Illusion | -0.26 pp | Ocean recovers main-deck HEROes | +0.10 pp |
| 11 | Judgment Dragon with two turns of Deck | +0.62 pp | Set Super Polymerization | +0.16 pp |
| 12 | Less restrictive Judgment wipe | +0.62 pp | King of the Swamp before Fusion | +0.55 pp |
| 13 | Compulsory on large ordinary monsters too | +0.55 pp | Birdman hand activation with Absolute Zero | +0.03 pp |

The combined screen gave **Lightsworn Judgment +6.74 pp** and **Fusion Heroes +8.43 pp**. The larger final development run used
1,024 games per reference opponent (12,288 pairs per policy), yielding
**Lightsworn +6.87 [+6.14, +7.59] pp** and **Heroes +9.08 [+8.28, +9.88] pp**. All
6,144 selected-screen scores and decision digests reproduced after cleanup.

Weak or harmful proposals were omitted, including reducing the general mill
reserve, unrestricted Glorious Illusion, broader normal-Tuner priorities,
contextual Lumina/Lyla revival, changing Celestia's tribute priority, bouncing
ordinary large monsters with Compulsory, Ocean restrictions, always favoring
Stratos searches, changing King of the Swamp timing, setting Super
Polymerization, and proactive Snowman Eater flips. The first Birdman proposal
changed a summon hook, but the pinned script offers a hand activation, so it
had no effect. The corrected activation experiment also showed no convincing
gain and was omitted. Preserving Absolute Zero even when outmatched was less
successful than the narrower winning-Fusion guard.

All screens finished without failed games or decision-limit draws.

## Search headroom

Both versions used seed `38130000`, eight games per reference opponent,
8/32/96-world stages, strict mode enabled, and foresight disabled.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Lightsworn Judgment | before | 96 | 56.2% | 79.2% | 22.9 pp |
| Lightsworn Judgment | after | 96 | 68.8% | 84.4% | 15.6 pp |
| Fusion Heroes | before | 96 | 56.2% | 77.1% | 20.8 pp |
| Fusion Heroes | after | 96 | 71.9% | 81.2% | 9.4 pp |

Each follow-up search uses a separate library with only its tested pilot
improved; the other remains at baseline. Both versions therefore face the
same 12 baseline opponents. Metadata records each library's hash and source
composition. An ordinary paired comparison reproduced all 192 old/new
standalone search scores and decision digests. Searches had no failed playouts
or limit draws.

Lightsworn search repeatedly chose Compulsory, Judgment Dragon, Ryko flips,
and Plaguespreader. Heroes search often delayed Fusion spells or summoned
Stratos first. Card-selection rules also came from policy and pinned-script
inspection: search does not explore card-selection alternatives.

These are small samples estimating headroom, not hard ceilings. Search has
privileged opposing-deck composition and engine random-state access. Its
one-step continuations depend on the pilot, so search is not a monotonic
upper bound as that pilot improves.

## Canonical ranking and checks

The full 31-policy round robin used seed `730000`, 256 games per unordered
pair: **119,040 games**, with no failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Lightsworn Judgment | #17 → #7 | 56.1% → 63.2% |
| Fusion Heroes | #13 → #5 | 60.0% → 67.8% |

All **103,936 games involving neither changed policy** reproduced both scores
and decision digests from the preceding Rock Block/Tele-DAD ranking. The
[tier list](../DECK-TIER-LIST.md) reproduced byte-for-byte from saved results.
Tournament rates differ from paired held-out rates because the seed suite
and opponent versions differ.

The workspace passed **157 tests**, including ten new regressions for
revival versus discard priorities, Plaguespreader costs and Synchro routes,
Judgment Dragon sequencing and deck-out limits, wipe costs, Ryko flips and
aliases, Compulsory targets, Stratos sequencing, Absolute Zero selection,
and retaining or replacing a Fusion according to the board.

- [Compact evidence](lightsworn-heroes-2026-10-06.summary.json)
- [Run metadata and hashes](lightsworn-heroes-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-lightsworn-heroes.summary.json)
- [Ranking metadata](round-robin-2026-10-06-lightsworn-heroes.metadata.json)

Raw JSONL, search traces, and frozen binaries remain outside Git under
`/tmp/ygo-lightsworn-heroes.2acwhlo7`. Their hashes and exact arguments are recorded.
