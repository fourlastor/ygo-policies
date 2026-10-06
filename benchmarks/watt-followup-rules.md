# Watt: surviving long enough to attack

This round builds on the [first Watt/Toon improvement](watt-toon-rules.md).
The baseline already uses Wattcube's permanent boost and prioritizes direct
attacks. Decks, engine, card scripts and database are unchanged. Harpie stays
in the ranking pool with its policy unchanged.

## Held-out validation

Each version played 512 games against each of the 30 other policies, with
identical seats, shuffles, policy randomness and baseline opponents. Seed
`9160000` was reserved until the candidate was selected; no further policy
tuning used these results. Draws count as half a win.

| Paired games per version | Before | After | Gain [paired 95% CI], percentage points |
|---:|---:|---:|---:|
| 15,360 | 20.96% | 28.18% | +7.22 [+6.70, +7.74] |

There were 1,428 improved outcomes and 319 regressions,
with **zero failed games or decision-limit draws**. Intervals are paired normal
approximations. This is an average gain across the roster, not a guarantee for
every matchup; Watt remains a weak deck.

## Changes retained

- **Use small monsters as blockers.** Remove the veto on setting Wattkid,
  Wattberyx, Wattkiwi and Wattmole. It made the pilot end turns with a legal
  blocker still in hand when those monsters could not fight the opposing board.
  The shared summon logic now considers those sets normally.
- **Keep Wattdragonfly's replacements alive.** When recruiting on the opponent's
  turn and outnumbered by face-up attackers, prefer another Dragonfly, then
  Wattlemur or Wattfox. This supplies more blockers or a useful destruction
  trigger instead of immediately spending the tutor on a vulnerable direct
  attacker. Special Summons of Watts on the opponent's turn use Defense Position.
- **Bring direct attackers out of Defense Position.** In Main Phase 1, when
  battle is available, use a legal position change for a direct attacker even
  if a larger opposing monster would discourage the generic repositioning rule.
- **Prefer replacement blockers under pressure.** Against a visible monster with
  at least 1500 ATK, setting Dragonfly or Lemur takes priority over an ordinary
  direct attacker. Keep the direct summon when its damage plus existing direct
  attackers can finish the duel this turn.

The follow-up search from the first round supplied examples of unused small
blockers, defensive sets preferred to direct summons, and direct attackers
left in Defense Position. Dragonfly's recruitment choices were checked against
its local card script and the selection code: this search does not explore
card-selection alternatives.

## Development measurements

Screening used 256 games against each of the 13 reference policies, seed
`8050000`: 3,328 paired games per variant. Independent changes over the starting
Watt policy gained:

| Change | Gain, percentage points |
|---|---:|
| Allow the four small monsters to be set | +4.66 |
| Dragonfly recruitment and defensive Special Summons | +1.44 |
| Reposition direct attackers | +0.66 |
| Broader offensive Wattkeeper timing | +0.18 |
| Apply equips after summons | +0.06 |

The first three together gained **+6.55 points** in screening. They were then
checked at 1,024 games per reference opponent, or 13,312 pairs. Adding the
pressure-based Dragonfly/Lemur summon priority, with a check for lethal direct
damage, gave a further **+0.32 [0.03, +0.61]**
points on that larger development set. The final package went from
**20.57% to 27.29%**, a gain of
**+6.72 points [6.18, 7.27]**.

Broader Wattkeeper timing and deferred equips were not retained. On top of the
three-rule package, early Waboku activation lost 0.27 points, broader Synchro
summoning added only 0.09 points, and avoiding Wattjustment on monsters with
useful effects lost 0.51 points. Those additions were also discarded. Complete
paired intervals and variant results are in the compact summary below.

## Search headroom

The same 104 discovery games (eight per reference opponent, seed `4510000`)
were searched again with the selected candidate, using default 8/32/96-world
stages and strict hidden-information checks.

| Version | Pilot alone | With search | Gap |
|---|---:|---:|---:|
| Before this round | 22.1% | 37.5% | 15.4 pp |
| After this round | 30.8% | 45.2% | 14.4 pp |

These small samples estimate headroom, not a hard ceiling. Search skips open
chains and positions facing an opposing Set monster, and leaves card selections
to the pilot. The new run used 211,424 playouts, with
0 failures and 0 decision-limit duels.
The held-out comparison is the evidence for the policy gain.

## Current ranking and verification

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from **119,040 games**
(256 per pair, seed `730000`). Watt is now **#25 at 28.0%** against
the complete roster, compared with #29 at 20.5% after the first Watt round.
There were no failures or decision-limit draws. Harpie remains included.

All **111,360 games not involving Watt** reproduced the preceding tournament's
response transcripts and outcomes exactly. Toon, Ojama and Destiny HERO source
files are unchanged. Rebuilding the ranking from saved results reproduced the
Markdown byte-for-byte.

**82 workspace tests passed**, including five new tests covering small blockers,
repositioning direct attackers, Dragonfly's defensive choices, summoned positions,
and defensive priorities that preserve lethal direct damage. Four of the new
tests were also run against the previous Watt source and reproduced the original
mistakes. A 480-pair check confirmed identical response transcripts between the
selected development library and the final build after comments, tests and the
explicit Main Phase 1 guard were completed.

- [Paired, screening and search results](watt-followup-2026-10-06.summary.json)
- [Source/binary metadata and raw-result hashes](watt-followup-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-watt-followup.summary.json)
- [Ranking metadata](round-robin-2026-10-06-watt-followup.metadata.json)

## Reproduction

Frozen libraries, source snapshots and raw results are local in
`/tmp/ygo-watt-followup.6kk6fhvf`. The baseline includes both previous policy
rounds; it is the final library recorded in the Watt/Toon report, before this
follow-up. Large JSONL and replay files remain outside Git.

Build libraries with `cargo build --release -p ygo-policies-ffi` and the runner
with `cargo build --release -p ygo-policies-bench`. Given `$BASELINE`,
`$CANDIDATE`, and a fresh `$OUT` directory:

```sh
target/release/policy-bench compare \
  --policies watt --opponents all --games 512 --seed 9160000 --workers 10 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" --output "$OUT/heldout.jsonl"

target/release/policy-bench search \
  --policies watt --opponents existing --games 8 --seed 4510000 \
  --worlds 8 --confirm 32 --final 96 --strict true --log true --record true \
  --library "$CANDIDATE" --workers 8 --output "$OUT/search.jsonl"

target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 12 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```
