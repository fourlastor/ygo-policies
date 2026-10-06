# Ojama and Destiny HERO policies

The two weakest pilots in the 2026-10-06 ranking were selected, keeping
both deck lists fixed. Harpie Sisters remains an unchanged opponent in the
31-deck ranking pool; its illegal list still needs replacement separately.

## Paired validation

Baseline: `bc8c0f76c349df214ddf4cbfa07a1860e85a8a63`, rebuilt against the
current vendored engine, scripts and database. Candidate: the Ojama and
Destiny HERO changes on `codex/ojama-destiny-policies`. Both versions face
the **baseline opponent**, with identical shuffles, seats and random seeds.
Draws count as half a win. Intervals are paired normal-approximation 95% CIs.

| Pilot | Games per version | Before | After | Change, percentage points [95% CI] |
|---|---:|---:|---:|---:|
| Ojama Brigade | 15,360 | 14.63% | 17.01% | +2.38 [+2.02, +2.74] |
| Destiny HEROes | 15,360 | 17.61% | 19.07% | +1.46 [+0.98, +1.94] |

This held-out run uses seed `3540000`, 512 games against each of all 30
other decks, including Harpie. There were no failed games or decision-limit
draws. The gains are averages over the roster, not improvements in every
matchup. Both decks remain weak.

Development used seed `2430000` against the 13 reference opponents. The
first screening samples were 128 games per opponent. The combined candidate
then played 512 per opponent (6,656 per pilot): Ojama gained **2.09 points
[1.55, 2.63]**, Destiny HERO **2.64 [1.92, 3.37]**. No further tuning used the
held-out results.

## Rules retained

Ojama:

- Blue searches Country and Ojamagic to establish the revival/search cycle,
  counting the first selected card while choosing the second. It searches
  Hurricane when the three names are available, and avoids redundant copies.
- Country revives King or Knight even without a spare hand full of Ojamas,
  and recycles Blue. Revival selection favors the fusion attacker, a missing
  Hurricane piece, then Blue rather than an unrelated Normal Ojama.
- With Country active, small Ojamas can be Normal Summoned and stand in
  Attack Position in Main Phase 1. Set Ojamas are flipped before judging
  battles under the stat swap. Red's multi-summon remains the first priority.

Destiny HERO:

- Summon or flip Doom Lord to remove a lone monster the current board cannot
  beat. Do not expose its 600 ATK to a second enemy; keep it in Attack
  Position so its banishing effect can work.
- Set Defender to stall attackers below 2700 ATK that the current board
  cannot beat. Defender is also allowed as the third body for Dogma/Plasma;
  the old unconditional veto prevented that play.
- Dunker sends Malicious or Dasher to the Graveyard even before burn is
  lethal. Malicious supplies a Tribute when Dasher is waiting in hand.
  Over Destiny still summons only for a finisher, because its monster dies
  in the End Phase. D - Time remains unused: this list has no Elemental HERO.
- D - Chain equips an attacker for lasting extra damage, including direct
  attacks, and can answer an incoming attack when 500 ATK turns it into a win.

## What the search contributed

Logged one-step searches use the default 8/32/96-world stages, strict hidden
information checks, seed `1420000`, eight games per reference opponent and
both seats: **104 games per pilot per search**. These are discovery seeds,
not the held-out validation above. The search offers an estimate of remaining
headroom, not a hard upper bound: it does not optimize card selections and
skips positions with an opponent's face-down monster under strict mode.

| Pilot | Version | Alone | With search | Gap |
|---|---|---:|---:|---:|
| Ojama | before | 15.4% | 24.0% | 8.7 pp |
| Ojama | after | 18.3% | 27.9% | 9.6 pp |
| Destiny HERO | before | 15.4% | 33.7% | 18.3 pp |
| Destiny HERO | after | 18.3% | 30.8% | 12.5 pp |

No search playout failed and no duel reached the decision limit. These small
samples describe the searched positions; the larger held-out comparison is
the evidence that the policy changes improve results.

Destiny HERO's observed gap narrowed; Ojama's did not. Ojama's search also
benefits from the improved card-selection rules, so a stronger ordinary
pilot need not reduce the difference from the search on top of it.

The logs exposed Country's missed summons/flips and Doom Lord's missed
removal opportunities, along with Defender, Dunker and D - Chain plays.
Reading the cards' local scripts and selection code revealed the Blue search
sequence, which this search cannot directly optimize.

Several plausible changes failed the initial paired screening: broadly
using Doom Lord against multiple monsters, activating Ojama Trio earlier
on the opponent's turn, and equipping D - Chain at any opportunity. Looser
Dogma/Plasma tribute rules also showed no clear gain. These were discarded.
Small individual gains were uncertain; the retained package was evaluated
as a whole on the larger development and held-out samples.

## Ranking after this round

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from a fresh
119,040-game round robin (256 games per pair, seed `730000`), with zero
failures or decision-limit draws. Destiny HERO ranks **29th at 20.4%** and
Ojama **31st at 17.7%** against the complete pool. These tournament rates
use different seeds from the held-out table and both updated pilots.
Rebuilding the Markdown from the saved results reproduced it byte-for-byte.
The canonical tier list now includes later rounds; the summary below preserves
this round's measurements.

- [Paired and search results](ojama-destiny-2026-10-06.summary.json)
- [Comparison/search metadata and raw-result hashes](ojama-destiny-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-ojama-destiny.summary.json)
- [Ranking metadata](round-robin-2026-10-06-ojama-destiny.metadata.json)

## Reproduction

Preserve the baseline shared library from the baseline revision, then build
the candidate with `cargo build --release -p ygo-policies-ffi -p ygo-policies-bench`.
With `$BASELINE` and `$CANDIDATE` naming those libraries and `$OUT` a fresh
local results directory:

```sh
# Held-out comparison: each candidate faces unchanged baseline opponents.
target/release/policy-bench compare \
  --policies ojama,destiny-hero --opponents all --games 512 --seed 3540000 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" --workers 8 \
  --output "$OUT/heldout.jsonl"

# Run once for each library to compare search headroom.
target/release/policy-bench search \
  --policies ojama,destiny-hero --opponents existing --games 8 --seed 1420000 \
  --library "$CANDIDATE" --workers 16 --log true --record true \
  --output "$OUT/search.jsonl"

# Fresh full ranking, including the unchanged Harpie deck.
target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 10 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```

Engine `1c49ef72b17126393399bdde44eb9077d882ff84`; CardScripts
`37f270dc813a12d123707ae255f2bda7922999c4`. Deck, database and binary identities
are recorded in the accompanying metadata. Raw JSONL, logged positions and
frozen libraries remain local in `/tmp/ygo-weak-policies.iyn01b00`; they are
not added to Git.

Validation: **73 workspace tests passed**, including five new regression tests
covering search selection, revival, position changes, defensive summons,
graveyard setup and D - Chain's battle threshold.
