# Arcana Force Fortune and Burn Princess

Arcana and Burn were the weakest remaining eligible entries in
[search-status.md](search-status.md): 29th and 27th in the preceding ranking.
This round builds on Arcana's earlier public coin-result handling. Both deck
lists and all other policies remain unchanged. Harpie stays in the measured
pool; its illegal list remains deferred for replacement and remeasurement.

## Held-out validation

Baseline: `11e7528c9f80eeae6c16422a78e9ceefe621fa31`. Candidate: the two policy
changes on `codex/arcana-burn-policies`. Both versions face the **baseline
opponent**, with identical shuffles, seats and policy randomness. Seed
`12590000` was reserved until policy selection was complete. No further tuning
used these results. Each version played 512 games against each of all 30
other decks. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Arcana Force Fortune | 15,360 | 20.73% | 21.82% | +1.09 [+0.84, +1.34] |
| Burn Princess | 15,360 | 24.83% | 31.52% | +6.69 [+6.17, +7.20] |

There were **zero failed games and zero decision-limit draws**. Intervals are
paired normal approximations. Arcana's gain is modest; Burn's is substantially
larger. These are average gains across the roster, not guarantees for every
matchup. In this held-out sample, Burn improved against all 30 opponents;
Arcana improved against 25, tied three and declined against two. The largest
Arcana decline was 0.59 points against Lightsworn. These per-matchup samples
are smaller than the aggregate; both decks remain weak.

## Rules retained

Arcana:

- Activate Solidarity when our Fairies are face-down too. Previously, a set
  Fairy could wait for enough ATK to flip while Solidarity waited for a face-up
  Fairy before activating. Account for the 800 ATK boost when choosing normal
  summons and flips. The boost requires a nonempty monster graveyard containing
  only Fairies; an empty or mixed-race graveyard does not qualify.
- Allow a 4000-ATK EX Ruler to replace three bodies when the opposing board
  matches or beats our strongest face-up monster but is smaller than the Ruler. The old
  rule rejected every three-body cost above 4500, even when those monsters
  could not break the opposing board. Face-down monsters are legal costs.

Burn:

- Keep Wave-Motion Cannon charging until its damage is lethal. The old policy
  normally spent it as soon as it reached 2000 damage. Other burn spells still resolve normally and
  can lower the opponent's LP enough to make the charged Cannon lethal.
- UFO Turtle recruits another Turtle when enemy face-up attackers outnumber
  our remaining bodies during the opponent's Battle Phase. Its summons are
  forced into Attack Position, so this preserves the replacement chain instead
  of immediately exposing Fire Princess. Otherwise, Princess keeps priority.
- Normal Summon Turtle when battle is available and it beats the visible
  opposing ATK. Previously, its higher Set score prevented these free attacks.
  Keep setting it against larger attackers and after the Battle Phase.

## Development measurements

Screening used seed `11480000`, 256 games per reference opponent: 3,328 paired
Arcana games and 3,072 Burn games per candidate. Burn is itself in the reference
pool, so it faces the other 12 references rather than all 13.

| Independent candidate | Gain in screening |
|---|---:|
| Arcana: Solidarity setup and projected boost | +0.48 pp |
| Arcana: EX Rulers break stalled boards | +0.39 pp |
| Burn: charge Cannon to at least 4000 instead of 2000 | +4.67 pp |
| Burn: Turtle recruits replacement Turtles | +0.72 pp |
| Burn: Turtle takes available attacks | +0.76 pp |

The final combined policies were checked at 1,024 games per reference
opponent: Arcana **+0.76 pp**
[+0.50, +1.02] over
13,312 pairs; Burn **+6.25 pp**
[+5.69, +6.81] over
12,288 pairs.

Several plausible changes were rejected. Aggressive Emperor/Empress summons
lost 1.17 points; early token-wall spells lost 0.90. Earlier Princess setup,
Book of Moon to suppress harmful coin effects, proactive dice, and early
Compulsory Evacuation showed no clear benefit. Recruiting chains of Shining
Angel also did not help Arcana in screening.

A narrower Emperor/Empress summon rule with Second Coin Toss support looked
promising initially, but its larger-run increment was only
+0.03 points
[-0.10, +0.16]; it was
removed. The same comparison also omitted an extra position-change rule whose
standalone screen changed no outcomes. Charging Cannon beyond 4000 to lethal
had the same overall score in the larger sample, with an incremental interval
[-0.23, +0.23]. The final rule
uses the simpler lethal check. Screening intervals are exploratory; the fresh
held-out comparison above is the confirmation.

## Search headroom

Logged strict searches used seed `10370000`, eight games per reference
opponent, and the default 8/32/96-world stages. Search helped reveal missed
Solidarity-assisted summons/flips, rejected Ruler summons, and premature Cannon
cash-outs. Turtle recruitment was checked against its card script and policy
selection code; search does not explore card-selection alternatives.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Arcana Force Fortune | before | 104 | 20.2% | 36.5% | 16.3 pp |
| Arcana Force Fortune | after | 104 | 24.0% | 38.5% | 14.4 pp |
| Burn Princess | before | 96 | 16.7% | 30.2% | 13.5 pp |
| Burn Princess | after | 96 | 28.1% | 35.4% | 7.3 pp |

These small discovery samples estimate headroom, not a hard ceiling or a
guaranteed gain achievable by an information-limited policy. Strict mode skips
positions with hidden opposing monsters, but search samples from the opponent's
real deck composition and preserves the engine's random state, including future
coin and die outcomes. These privileges belong to the development search, not
the ordinary policies or their held-out comparisons. Search is only one step
on top of the current pilot. The follow-up Arcana search also faces the
improved Burn reference opponent; the held-out comparison above keeps opponents
fixed. No search playout failed and no searched duel hit the decision limit.

## Ranking refresh and validation

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from a full
119,040-game round robin, 256 games per pair, seed `730000`, with all 31 decks.
It had zero failures and zero decision-limit draws.

| Deck | Rank before → after | Measured-pool score before → after |
|---|---:|---:|
| Arcana Force Fortune | #29 → #29 | 21.2% → 22.0% |
| Burn Princess | #27 → #24 | 24.7% → 31.6% |

All **103,936 games involving neither changed policy** reproduced the previous
tournament's outcomes and transcript digests exactly. Rebuilding the tier list
from the saved tournament reproduced the Markdown byte-for-byte.

**87 workspace tests passed**, including five new regression tests for
Solidarity's setup and graveyard conditions, Ruler summons, Cannon timing and
lethal damage, Turtle recruitment, and Turtle summon positions.

- [Paired, screening and search results](arcana-burn-2026-10-06.summary.json)
- [Run metadata and raw-result hashes](arcana-burn-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-arcana-burn.summary.json)
- [Ranking metadata](round-robin-2026-10-06-arcana-burn.metadata.json)

## Reproduction

Build and preserve the baseline shared library at the baseline revision, then
build the candidate with `cargo build --release -p ygo-policies-ffi -p ygo-policies-bench`.
With `$BASELINE` and `$CANDIDATE` naming those libraries and `$OUT` a fresh directory:

```sh
target/release/policy-bench compare \
  --policies arcana,burn --opponents all --games 512 --seed 12590000 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" --workers 10 \
  --output "$OUT/heldout.jsonl"

# Repeat with each library for before/after search headroom.
target/release/policy-bench search \
  --policies arcana,burn --opponents existing --games 8 --seed 10370000 \
  --library "$CANDIDATE" --workers 12 --strict true --log true --record true \
  --output "$OUT/search.jsonl"

target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 10 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```

Engine, scripts, database and deck identities are recorded in the metadata.
Large JSONL logs, recorded positions and frozen libraries remain local in
`/tmp/ygo-arcana-burn.m5t0260g`; they are not added to Git.
