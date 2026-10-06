# Gishki and Karakuri

Gishki and Karakuri were the weakest remaining eligible entries in
[search-status.md](search-status.md), ranked 23rd and 20th. Baseline:
`b51aeb0a5d8443d113dd82d113553b7d19af96ba`. This round changes these two policies on
`codex/gishki-karakuri-policies`. Harpie remains in the 31-deck ranking pool;
its illegal list still excludes it from policy improvements.

## Held-out validation

Seed `22490000` was reserved until rule selection was complete. Each version
played 512 games against each of all 30 other decks, with the same shuffles,
seats, policy randomness, and **baseline opponent policy**. No tuning used
these held-out results. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Gishki | 15,360 | 39.90% | 49.81% | +9.91 [+9.24, +10.58] |
| Karakuri | 15,360 | 44.41% | 51.30% | +6.88 [+6.29, +7.47] |

There were **zero failed games**. Karakuri had no decision-limit draws;
Gishki had one candidate game capped at 4,096 decisions. Intervals are paired
normal approximations. Gishki improved against 30 opponents, tied 0, and declined against 0. Karakuri improved against 30 opponents, tied 0, and declined against 0. Individual matchup samples are smaller than the aggregate; see the summary for their results.

The capped Gishki game against Destiny HEROes (seed `1578024060`, seat 1)
reproduced its exact decision digest. Raising its diagnostic cap to 16,384
let it finish at **4,799 decisions on turn 90**, with Gishki losing by deck-out.
It was a long recycling duel, not a decision loop. The main result retains
the original fixed-cap draw; treating it as a loss instead changes the gain
by only 0.0033 percentage points, still **+9.91 pp**. This diagnostic did not
change the policy or the held-out protocol.

## Rules retained

Gishki:

- Search for Aquamirror when a ritual monster is already in hand but the
  mirror is missing; search for a ritual monster when that piece is missing.
  With both available, prefer Shadow as a complete ritual tribute when a
  legal search offers it.
- Use Aquamirror's Graveyard recovery even when another ritual monster is
  already in hand. The recovered card can fund another summon or Soul Ogre's
  discard effect.
- Flip a set Ariel when the legal action is offered, obtaining its search
  without waiting to be attacked. Leave a face-up defensive Ariel alone.
- Use Tetrogre's effect during our Main Phase. The opponent may discard to
  negate it; otherwise the existing legal selection chooses the monster to
  send from our own Deck. This rule does not inspect the opposing hand or
  choose using hidden opposing cards.
- Use Meditation in our Main Phase to recover resources for the current
  turn, while retaining the existing opponent-End-Phase response.

Karakuri:

- Flip a set Sazank when an opposing face-up monster is available for its
  mandatory removal effect. Avoid flipping it into a board with only our
  own legal targets.
- Prioritize normal-summoning Nishipachi or Saizan when an existing face-up
  non-Tuner Machine supplies the remaining Levels for a Level 7 or 8 Synchro.
- Score position-change targets by the resulting public battle position:
  expose a weaker battle stat, move a small compulsory attacker to Defense,
  or bring a strong defender into Attack Position. A set Sazank can also be
  flipped for removal. Unrevealed opposing monsters are not scored using
  hidden identities or stats.
- Special Summon Cyber Dragon before the normal summon, preserving its
  empty-own-field summoning window and making it available as Synchro material.

All rules use the seat's permitted observation, public printed card data,
and legal choices. The ordinary policies have no engine snapshot, opposing
hidden identities, deck order, or future random outcomes. Shared policy code,
deck lists, and vendor pointers are unchanged.

## Development measurements

Screening used seed `21380000`, 256 games per reference opponent: 3,072 pairs
for Gishki and 3,328 for Karakuri. Gishki belongs to the 13-policy reference
pool and excludes its self-match; Karakuri faces all 13.

| Screen | Gishki candidate | Gain | Karakuri candidate | Gain |
|---|---|---:|---|---:|
| 1 | Find missing ritual piece | +2.15 pp | Flip Sazank | +1.68 pp |
| 2 | Recover spare ritual | +1.50 pp | Duplication with one free zone | -0.03 pp |
| 3 | Flip Ariel | +0.78 pp | Recruitment ignores hand tuners | -1.71 pp |
| 4 | Favor Level 6 rituals | -5.08 pp | Normal summon for Level 7/8 Synchro | +2.49 pp |
| 5 | Shadow searches past Forbidden Arts | +0.23 pp | Spend Anatomy at one counter | -0.51 pp |
| 6 | Prioritize Noellia | -0.13 pp | Battle if any attacker is stronger | +0.15 pp |
| 7 | Use Tetrogre | +3.16 pp | Position-target scoring | +2.28 pp |
| 8 | Earlier Meditation | +1.11 pp | Position scoring + broader Burei use | +2.37 pp |
| 9 | Normal summon spare support | +0.10 pp | Prefer Shoguns over generic Synchros | +0.00 pp |
| 10 | Unchanged | +0.00 pp | Battle if visible board can be cleared | +0.03 pp |
| 11 | Unchanged | +0.00 pp | Cyber Dragon before normal summon | +1.65 pp |
| 12 | Unchanged | +0.00 pp | Recruit a Synchro partner | +0.21 pp |

The combined screen gave **Gishki +9.80 pp** and **Karakuri +7.54 pp**. The final larger development run used
1,024 games per reference opponent: 12,288 Gishki pairs and 13,312 Karakuri
pairs. Its results were **Gishki +9.52 [+8.76, +10.28] pp** and
**Karakuri +7.81 [+7.15, +8.47] pp**.

Screening results guided selection and are exploratory, not independent
confirmation. The held-out results above provide that check. Changes with
no convincing benefit were omitted, including the broad Shogun preference,
spare Gishki support summons, earlier Anatomy cash-out, and altered battle
entry. The smaller-ritual preference was harmful: Soul Ogre's removal remains
central to the policy. The first recruitment experiment ignored tuners in
hand and lost points; it was discarded. Broader Burei activation added only 0.09 points over the
position-target rule in its screen, so that extra activation rule was omitted.

## Search headroom

Both searches used seed `20270000`, eight games per reference opponent,
8/32/96-world stages, strict mode enabled, and foresight disabled. Logged
search deviations exposed missed Ariel/Sazank flips, unused Tetrogre and
Meditation effects, and better tuner/summon choices. Missing-piece searches
and position-target scoring also came from code and card-script inspection;
search does not explore card-selection alternatives.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Gishki | before | 96 | 26.0% | 43.8% | 17.7 pp |
| Gishki | after | 96 | 39.6% | 50.0% | 10.4 pp |
| Karakuri | before | 104 | 48.1% | 71.2% | 23.1 pp |
| Karakuri | after | 104 | 46.2% | 72.1% | 26.0 pp |

These small samples estimate headroom, not hard ceilings or gains guaranteed
achievable by an ordinary policy. Strict search skips hidden opposing monsters
and open chains, but retains privileged opposing-deck composition and engine
random-state access. The Gishki search faces the same opponent policies before
and after; Karakuri's follow-up search also faces the improved Gishki pilot.
The paired development and held-out comparisons keep opponents fixed.

Karakuri's lower standalone result in the follow-up search is driven by
its changed Gishki opponent: it goes from 5/8 to 2/8 wins in that matchup.
Against the 12 unchanged reference opponents, its standalone result rises
from 46.9% to 47.9%, while search rises from 70.8% to 71.9%. These small
samples still leave substantial Karakuri headroom; the much larger fixed-
opponent development and held-out samples are the evidence for the retained
policy improvements.

Both searches completed with zero failed playouts and zero decision-limit
draws.

## Canonical ranking

The full round robin used all 31 policies, 256 games per unordered pair,
seed `730000`: **119,040 games**, with no failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Gishki | #23 → #19 | 39.6% → 49.5% |
| Karakuri | #20 → #18 | 43.7% → 50.4% |

All **103,936 games involving neither Gishki nor Karakuri** exactly reproduced
both scores and decision digests from the preceding Crystal/Morphtronic
ranking. The generated [tier list](../DECK-TIER-LIST.md) was reproduced
byte-for-byte from the saved results. These all-pool rates differ from the
paired held-out rates because the seed suite and opponent versions differ.

## Validation and artifacts

The workspace passed **114 tests**, including new policy regressions for
missing ritual pieces, spare ritual recovery, flip timing, tuner selection,
position targets, and activation windows.

- [Compact evidence](gishki-karakuri-2026-10-06.summary.json)
- [Run metadata and source/raw-result hashes](gishki-karakuri-2026-10-06.metadata.json)
- [Full ranking summary](round-robin-2026-10-06-gishki-karakuri.summary.json)
- [Ranking metadata](round-robin-2026-10-06-gishki-karakuri.metadata.json)

Large JSONL files, search traces, and frozen libraries remain outside Git in
`/tmp/ygo-gishki-karakuri.crciybpz`. The evidence records exact arguments,
engine/scripts/database/deck fingerprints, and source/library hashes.
