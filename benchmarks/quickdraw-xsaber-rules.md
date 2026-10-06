# Quickdraw Plants and X-Sabers

Quickdraw Plants and X-Sabers were the weakest remaining eligible entries
in [search-status.md](search-status.md), ranked 17th and 16th. Baseline:
`ae5aad66fcc02be59a40e79361a550b5502e822e`. This round changes these two policies on
`codex/quickdraw-xsaber-policies`. Harpie remains in the 31-deck ranking pool;
its illegal list still excludes it from policy improvements.

## Held-out validation

Seed `25820000` was reserved until rule selection was complete. Each version
played 512 games against each of all 30 other decks, with the same shuffles,
seats, policy randomness, and **baseline opponent policy**. No tuning used
these held-out results. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Quickdraw Plants | 15,360 | 51.32% | 56.70% | +5.38 [+4.82, +5.93] |
| X-Sabers | 15,360 | 52.04% | 59.15% | +7.12 [+6.51, +7.73] |

There were **zero failed games and zero decision-limit draws**. Intervals
are paired normal approximations. Quickdraw Plants improved against 29 opponents, tied 0, and declined against 1. X-Sabers improved against 30 opponents, tied 0, and declined against 0. Individual matchup samples are smaller than the aggregate; see the summary for their results.

Quickdraw's only negative matchup point estimate was Ojama: -0.39 pp
(512 pairs, paired 95% interval [-2.27, +1.49] pp).
That small difference is inconclusive; it was not used for further tuning.

## Rules retained

Quickdraw Plants:

- Special Summon Quickdraw when Level Eater is in hand or the Graveyard and
  Junk Warrior remains in the Extra Deck. If necessary, deliberately discard
  Level Eater: it can revive by reducing Quickdraw from Level 5 to 4, making
  Junk Warrior with the resulting Level 4 Tuner and Level 1 non-Tuner.
  Reserve two monster zones for the line.
- Normal Summon Bulb, Spore, or Plaguespreader when the existing Level-based
  heuristic identifies a useful Synchro payoff. Previously all three were
  vetoed as Normal Summons.
- Include Formula Synchron and Armory Arm in that heuristic, allowing smaller
  setup plays rather than requiring a high-value boss immediately. This also
  applies to the existing Graveyard tuner revival rules.
- Junk Synchron's revival prefers a non-Tuner partner. Reviving another Tuner
  with its effect negated still leaves it a Tuner, often stranding the pair.

X-Sabers:

- Use the previously unused One for One to recruit Ragigura or Palomuro,
  preserving the Normal Summon for another body.
- Flip a set X-Saber when one other X-Saber is face-up and Faultroll is in
  hand, meeting Faultroll's summon requirement.
- In legal Deck searches, prefer low-Level partners when Faultroll is already
  in hand instead of collecting another unplayable large monster.
- Set Pashuul when no immediate Synchro play is available, using its battle
  protection to keep a body on the field.
- Use Saber Hole against small opposing summons too. Low ATK does not make
  an engine monster harmless; the old 1,900-ATK threshold missed these.

All rules use the seat's permitted observation, public printed card data,
and legal choices. Ordinary policies have no engine snapshot, opposing
hidden identities, deck order, or future random outcomes. Shared policy code,
deck lists, and vendor pointers are unchanged.

## Development measurements

Each isolated screen used seed `24710000`, 256 games per reference opponent:
3,328 pairs per policy against all 13 reference decks.

| Screen | Quickdraw candidate | Gain | X-Saber candidate | Gain |
|---|---|---:|---|---:|
| 1 | Normal summon useful tuners | +2.22 pp | Ragigura / Faultroll recovery | +0.15 pp |
| 2 | Allow small setup Synchros | +1.29 pp | Flip a second X-Saber for Faultroll | +0.39 pp |
| 3 | Lonefire with full zones | +0.21 pp | Earlier Gottoms discard | +0.06 pp |
| 4 | Cycle Lonefire before Tytannial | -0.03 pp | Pashuul special summons in defense | -0.03 pp |
| 5 | Junk revives a non-tuner | +0.30 pp | Search small bodies with Faultroll in hand | +0.42 pp |
| 6 | Quickdraw with hand Dandylion | +0.09 pp | Normal summon more tuners | +0.15 pp |
| 7 | Broader One for One use | -0.63 pp | Use One for One | +2.91 pp |
| 8 | Broader Level Eater use | +0.33 pp | Raise Naturia Beast priority | +0.33 pp |
| 9 | Quickdraw / Level Eater line | +3.09 pp | Raise Trishula priority | +0.33 pp |
| 10 | Unchanged | +0.00 pp | Raise Stardust priority | -0.39 pp |
| 11 | Unchanged | +0.00 pp | Recruit a Synchro partner | +0.00 pp |
| 12 | Subset material sums / restrictions | +0.18 pp | Saber Hole against small summons | +1.74 pp |
| 13 | Spore cost for Synchro level | +0.09 pp | Set Pashuul | +3.00 pp |

The selected combined screen gave **Quickdraw Plants +7.48 pp** and **X-Sabers +7.72 pp**. The final larger development
run used 1,024 games per reference opponent, 13,312 pairs per policy. It gave
**Quickdraw +6.24 [+5.61, +6.87] pp** and **X-Sabers +7.88 [+7.18, +8.58] pp**.

Screening guided selection and is exploratory, not independent confirmation.
The held-out results provide that check. Flat or harmful changes were omitted,
including Lonefire cycling, broader One for One use in Quickdraw, Pashuul's
special-summon position, and broad boss-priority changes.

Two additional combined Quickdraw experiments checked the tuner heuristic:
subset material sums with named-material restrictions scored +6.58 pp over
baseline, and restrictions with the original one-partner scan scored +6.97 pp.
Both underperformed the selected +7.48 pp combination, so neither was retained.
The existing Level-based helper remains a rough planning heuristic, not a
complete Synchro legality solver; actual summon choices always come from the
engine. All screening runs had zero failures and zero decision-limit draws.

## Search headroom

Both searches used seed `23600000`, eight games per reference opponent,
8/32/96-world stages, strict mode enabled, and foresight disabled. Both pilots
face the same 13 unchanged reference opponents before and after.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Quickdraw Plants | before | 104 | 56.7% | 82.7% | 26.0 pp |
| Quickdraw Plants | after | 104 | 60.6% | 79.8% | 19.2 pp |
| X-Sabers | before | 104 | 49.0% | 79.8% | 30.8 pp |
| X-Sabers | after | 104 | 61.5% | 80.8% | 19.2 pp |

The before-search logs repeatedly showed missed Quickdraw and tuner summons,
unused X-Saber One for One, and passed Saber Hole windows. One concrete
Quickdraw position against Burn (seed `2095837528`, decision 160) had an empty
field and both Quickdraw and Level Eater in hand: the old pilot ended its
turn while search Special Summoned Quickdraw. Search does not explore
card-selection alternatives; the explicit Level Eater discard, Junk revival
choice, and Faultroll search priorities came from reading those positions,
policy code, and card scripts.

These small samples estimate headroom, not hard ceilings or gains guaranteed
achievable by an ordinary policy. Strict search skips hidden opposing monsters
and open chains, but retains privileged opposing-deck composition and engine
random-state access. Both searches had zero failed playouts and zero
decision-limit draws.

On this small sample, searched Quickdraw wins 83/104 games after the change,
compared with 86/104 before, while the standalone pilot rises from 59 to 63
wins. A one-step search using the current pilot for its continuations is not
a monotonic upper bound when that pilot changes. The much larger fixed-
opponent held-out comparison measures the ordinary policy's improvement.

## Canonical ranking

The full round robin used all 31 policies, 256 games per unordered pair,
seed `730000`: **119,040 games**, with no failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Quickdraw Plants | #17 → #14 | 51.6% → 57.1% |
| X-Sabers | #16 → #10 | 52.4% → 59.9% |

All **103,936 games involving neither Quickdraw nor X-Sabers** exactly reproduced
both scores and decision digests from the preceding Gishki/Karakuri ranking.
The generated [tier list](../DECK-TIER-LIST.md) was reproduced byte-for-byte
from the saved results. These rates differ from the paired held-out rates
because the seed suite and opponent versions differ.

## Validation and artifacts

The workspace passed **122 tests**, including eight new policy regressions
for the Level Eater discard plan, small Synchro setup, Junk revival targets,
One for One, Faultroll flips and searches, Pashuul defense, and Saber Hole.

- [Compact evidence](quickdraw-xsaber-2026-10-06.summary.json)
- [Run metadata and source/raw-result hashes](quickdraw-xsaber-2026-10-06.metadata.json)
- [Full ranking summary](round-robin-2026-10-06-quickdraw-xsaber.summary.json)
- [Ranking metadata](round-robin-2026-10-06-quickdraw-xsaber.metadata.json)

Large JSONL files, search traces, and frozen libraries remain outside Git in
`/tmp/ygo-quickdraw-xsaber.hzpzye21`. The evidence records exact arguments,
engine/scripts/database/deck fingerprints, and source/library hashes.
