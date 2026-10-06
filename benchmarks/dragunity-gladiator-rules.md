# Dragunity and Gladiators

Dragunity Flight and Fight, Gladiators! were the weakest remaining eligible
entries in [search-status.md](search-status.md), ranked 17th and 16th.
Baseline: `6c0f6ba0aeee10ea0f3766706e3f41baae78d897`. This round changes these two policies on
`codex/dragunity-gladiator-policies`. Harpie remains in the 31-deck ranking
pool; its illegal list still excludes it from policy improvements.

## Held-out validation

Seed `29150000` was reserved until rule selection was complete. Each version
played 512 games against each of all 30 other decks, with identical shuffles,
seats, policy randomness, and **baseline opponent policy**. No tuning used
these held-out results. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Dragunity Flight | 15,360 | 54.47% | 63.92% | +9.45 [+8.80, +10.10] |
| Fight, Gladiators! | 15,360 | 55.83% | 62.46% | +6.63 [+6.01, +7.25] |

There were **zero failed games and zero decision-limit draws**. Intervals
are paired normal approximations. Dragunity Flight improved against 30 opponents, tied 0, and declined against 0. Fight, Gladiators! improved against 27 opponents, tied 1, and declined against 2. Individual matchup samples are smaller than the aggregate; see the summary for their results.

Gladiators had two negative matchup point estimates (512 pairs each):

- rock-block: 45.31% → 42.38%, -2.93 pp [-6.41, +0.55]
- verdict: 8.79% → 8.40%, -0.39 pp [-2.49, +1.71]

These are reported without further tuning on the held-out sample.

## Rules retained

Dragunity:

- Prefer Vajrayana when Phalanx is on our field or in our Graveyard. Its
  equip and Phalanx's revival provide a route to a Level 8 Synchro, rather
  than stopping at Gaia Knight's higher immediate ATK.
- Discard the first Phalanx into the Graveyard instead of preserving it over
  less useful hand cards. Once a Phalanx is already there, use the existing
  discard priorities.
- Use Dragon Ravine with one card left in hand. Its cost needs one discard,
  not two cards remaining before activation.
- Use Cards of Consonance with any single eligible tuner, rather than
  requiring Phalanx specifically or two tuners in hand. The legal activation
  comes from the engine; the policy does not bypass the card's cost filter.
- Use Icarus Attack during the Main Phase and response windows when the
  opponent has at least two cards, avoiding a planned activation that would
  need to target our own field to reach the mandatory two targets.
- Prefer Normal Summoning Legionnaire over Dux when Aklys is in our Graveyard
  and the opposing best attack exceeds 2,100, favoring removal against a
  large attacker.

Gladiators:

- Separate searches into hand from special summons out of the Deck. A hand
  search prefers playable Normal Summons, with Bestiari preferred when it
  completes a contact Fusion. Tag-ins retain their effect-based priorities.
- Activate Prisma when an existing Gladiator or a Test Tiger in hand can
  use its copied name. Reveal Gyzarus to send and copy Bestiari when that
  legal reveal is offered. Unrestricted Prisma use was not retained.
- Normal Summon even a small Gladiator when Test Tiger can immediately tag
  it out. The old blanket veto stranded these starts in hand.
- Prefer Gyzarus before Heraklinos when the opposing field has removal targets.
- Tag into Darius when Bestiari is in our Graveyard, Gyzarus remains in the
  Extra Deck, and two monster zones are free. Darius's revival then prefers
  Bestiari, preparing the named material for contact Fusion.
- Tag into Equeste to recover War Chariot when it is in our Graveyard and
  another copy is not already in hand, sustaining the monster-negation cycle.

All rules use the seat's permitted observation, public printed card data,
and legal choices. Ordinary policies have no engine snapshot, opposing
hidden identities, deck order, or future random outcomes. Shared policy code,
deck lists, and vendor pointers are unchanged.

## Development measurements

Each isolated screen used seed `28040000`, 256 games per reference opponent:
3,072 pairs per policy. Both policies belong to the 13-deck reference pool
and exclude their own self-match.

| Screen | Dragunity candidate | Gain | Gladiator candidate | Gain |
|---|---|---:|---|---:|
| 1 | Vajrayana with Phalanx | +2.77 pp | Unrestricted Prisma use | -0.07 pp |
| 2 | Discard first Phalanx | +1.73 pp | Separate hand searches from tag-ins | +2.86 pp |
| 3 | Ravine with one hand card | +2.96 pp | Darius revives Bestiari | +0.20 pp |
| 4 | Consonance with one eligible tuner | +1.40 pp | Tag Darius with Bestiari in GY | +0.75 pp |
| 5 | Ravine only for missing pieces | +0.13 pp | Prisma only with a combo partner | +0.98 pp |
| 6 | Ravine always prioritizes missing Phalanx | -0.62 pp | Normal summon to enable Test Tiger | +1.24 pp |
| 7 | Legionnaire equips Aklys first | +0.07 pp | Gyzarus before Heraklinos | +0.52 pp |
| 8 | Search Dux over Legionnaire | +0.10 pp | Explicit Shrink battle target | +0.20 pp |
| 9 | Use Icarus Attack | +1.99 pp | Shrink protects defenders | +0.20 pp |
| 10 | Use Vajrayana attack boost | +0.00 pp | Flip to enable Test Tiger | +0.26 pp |
| 11 | Broader Mystletainn summons | +0.10 pp | Offensive Enemy Controller | +0.23 pp |
| 12 | Legionnaire against large attackers | +0.42 pp | Small Gladiators attack empty boards | +0.62 pp |
| 13 | Unchanged | +0.00 pp | Equeste recycles War Chariot | +3.42 pp |

The combined screen gave **Dragunity Flight +10.55 pp** and **Fight, Gladiators! +8.37 pp**. The final larger development run
used 1,024 games per reference opponent: 12,288 pairs per policy. It gave
**Dragunity +10.34 [+9.57, +11.11] pp** and **Gladiators +8.10 [+7.35, +8.85] pp**.

Screening guided rule selection and is exploratory, not independent
confirmation. The held-out sample provides that check. Weak or harmful
changes were omitted, including always milling Phalanx with Ravine,
restricting Ravine to missing pieces, the extra Vajrayana boost rule,
broader Mystletainn summons, Shrink changes, offensive Enemy Controller,
and broader small-Gladiator attacks. The revival of Bestiari was retained
alongside Darius recruitment as a complete contact-Fusion plan.

All screening runs completed without failures or decision-limit draws.
The selected screen's 6,144 paired games exactly reproduced their scores
and decision digests in the larger final development run after code cleanup.

## Search headroom

Both searches used seed `26930000`, eight games per reference opponent,
8/32/96-world stages, strict mode enabled, and foresight disabled.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Dragunity Flight | before | 96 | 56.2% | 68.8% | 12.5 pp |
| Dragunity Flight | after | 96 | 63.5% | 81.2% | 17.7 pp |
| Fight, Gladiators! | before | 96 | 40.6% | 70.8% | 30.2 pp |
| Fight, Gladiators! | after | 96 | 51.0% | 70.8% | 19.8 pp |

Both pilots face the same 12 **baseline opponents** before and after. Because
both are reference opponents, each follow-up search uses a separately frozen
library: only its tested pilot has the new rules; the other policy remains at
baseline. The full improved library is used for paired comparisons and the
canonical ranking. Metadata records each search library's hash and exact
source composition. An ordinary paired comparison on all 192 search seeds
also reproduced the old and new standalone search-game scores and decision
digests exactly.

The search logs repeatedly chose Vajrayana and Icarus Attack for Dragunity,
and Prisma, alternate summons, and defensive timing for Gladiators. Card
selection priorities were developed by reading those positions, policy code,
and the pinned scripts; search itself does not explore card-selection
alternatives.

These small samples estimate remaining headroom, not hard ceilings or gains
guaranteed achievable by an ordinary policy. Strict search skips hidden
opposing monsters and open chains, but retains privileged opposing-deck
composition and engine random-state access. A one-step search that uses the
pilot for its continuations is not a monotonic upper bound when the pilot
changes. Both searches completed with zero failed playouts and zero
decision-limit draws.

## Canonical ranking

The full round robin used all 31 policies, 256 games per unordered pair,
seed `730000`: **119,040 games**, with no failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Dragunity Flight | #17 → #7 | 53.7% → 63.9% |
| Fight, Gladiators! | #16 → #8 | 55.7% → 63.0% |

All **103,936 games involving neither Dragunity nor Gladiators** exactly
reproduced both scores and decision digests from the preceding Quickdraw/
X-Saber ranking. The generated [tier list](../DECK-TIER-LIST.md) was reproduced
byte-for-byte from the saved results. These rates differ from the paired
held-out rates because the seed suite and opponent versions differ.

## Validation and artifacts

The workspace passed **134 tests**, including 12 new regressions covering
Vajrayana, Phalanx discards, Ravine and Consonance costs, Icarus's two-target
requirement, Legionnaire timing, hand searches, Prisma, Test Tiger starts,
contact-Fusion priority, Darius revival, and War Chariot recovery.

- [Compact evidence](dragunity-gladiator-2026-10-06.summary.json)
- [Run metadata and source/raw-result hashes](dragunity-gladiator-2026-10-06.metadata.json)
- [Full ranking summary](round-robin-2026-10-06-dragunity-gladiator.summary.json)
- [Ranking metadata](round-robin-2026-10-06-dragunity-gladiator.metadata.json)

Large JSONL files, search traces, and frozen libraries remain outside Git in
`/tmp/ygo-dragunity-gladiator.blg0fy39`. The evidence records exact arguments,
engine/scripts/database/deck fingerprints, and source/library hashes.
