# Crystal Beast and Morphtronic

Crystal Beast and Morphtronic were the weakest remaining eligible entries in
[search-status.md](search-status.md), ranked 23rd and 22nd. Baseline:
`314cf3d8867c8b5213a7cda423d4bc4c792f2ecd`. The candidate changes these two policies on
`codex/crystal-morphtronic-policies`; deck lists, shared policy code, and other
pilots are unchanged. Harpie stays in the ranking pool, with policy work
still deferred until its illegal list is replaced and remeasured.

## Held-out validation

Seed `19160000` was reserved until rule selection was complete. Each version
played 512 games against each of all 30 other decks. Both versions face the
**baseline opponent**, with identical shuffles, seats and policy randomness.
No tuning used these held-out results. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Crystal Beast | 15,360 | 32.02% | 41.20% | +9.18 [+8.48, +9.88] |
| Morphtronic | 15,360 | 37.34% | 43.67% | +6.33 [+5.75, +6.90] |

There were **zero failed games and zero decision-limit draws**. Intervals are
paired normal approximations. These are average gains across the roster,
not guarantees for each individual matchup. Both policies improved against all
30 opponents in this sample. Individual matchups have smaller samples than
the aggregate; their results are in the linked summary.

## Rules retained

Crystal Beast:

- Prefer Ruby Carbuncle for a Special Summon when at least two other Beasts
  are stored and three monster zones are free. Ruby can bring out both
  companions. The old linear bonus usually favored Pegasus instead. A Ruby
  summoned from the backrow does not count itself among those companions.
- Spend Crystal Tree at one counter and count the zone freed when Tree
  sends itself as cost. The old rule waited for two counters and required
  unnecessary empty zones, missing useful placements.
- Use Crystal Abundance against two or more opposing cards. Its mass removal
  also rebuilds our board from the Graveyard, so comparing current monster
  strength understated its value. Previously it required three opposing cards
  and an opposing field at least as strong as ours.
- Use Crystal Release to increase winning or direct-attack damage, rather
  than only when its 800 ATK crosses an opposing ATK threshold.
- Normal Summon a spare Ruby when stronger normal summons are unavailable.
  It supplies a body and can become a stored Beast after destruction; the
  old rule vetoed every normal summon of Ruby.
- Permit Malefic Rainbow Dragon under either player's face-up Field Spell.
  Its card script requires any face-up Field Spell, not specifically our own
  Rainbow Ruins. A set Field Spell or ordinary backrow card is insufficient.

Morphtronic:

- Prioritize Scopen over Radion when a Level 4 Morphtronic is in hand. This
  enables Scopen's free summon and the deck's Level 7 Synchro line.
- Otherwise prioritize Celfon over the weaker setup choices, including
  Remoten. Scopen with a Level 4 partner retains priority over Celfon.
- Summon Power Tool Dragon in Attack Position during our Main Phase 1,
  leaving attacks available for its Equip Spells even when its printed
  2300 ATK is initially below the opposing monster's ATK.
- Equip Morphtronic Cord before a planned position change when opposing
  backrow is present, then aim its destruction at that backrow. Avoid adding
  another face-up Cord. Power Tool Dragon cannot carry Cord, so its position
  changes do not trigger this setup rule.

All rules use permitted seat observations, our own cards, and legal choices.
None reads engine snapshots, opposing hidden card identities, deck order, or
future dice results. In particular, Celfon's priority does not depend on the
result of its upcoming roll.

## Development measurements

Screening used seed `18050000`, 256 games per reference opponent: 3,072 paired
games per pilot per candidate. Both pilots belong to the 13-policy reference
pool, so each excludes its own self-match.

| Independent candidate | Screening gain |
|---|---:|
| Crystal: Ruby summons two companions | +1.95 pp |
| Crystal: Tree at one counter and its freed zone | +1.73 pp |
| Crystal: Abundance against two cards | +1.56 pp |
| Crystal: Release for extra damage | +0.72 pp |
| Crystal: spare Ruby normal summon | +1.86 pp |
| Crystal: Malefic under either Field Spell | +0.62 pp |
| Morphtronic: prioritize Scopen with a Level 4 | +3.12 pp |
| Morphtronic: prioritize Celfon | +1.37 pp |
| Morphtronic: Power Tool in Attack Position | +0.65 pp |
| Morphtronic: Cord before position changes | +1.46 pp |

The combined screen improved Crystal by **8.01 points** and Morphtronic by
**7.32 points**. The final build was then checked at 1,024 games per reference
opponent, 12,288 pairs per pilot: Crystal **+8.69 [+7.92, +9.46] pp** and
Morphtronic **+6.47 [+5.80, +7.14] pp**.

The Celfon-only screen gave it priority over Scopen as well. The combined
policy places it below Scopen's summon with a known Level 4 partner. A final review also
restricted Cord's setup to legal Morphtronic holders; a regression covers the
Power Tool Dragon case. The larger development comparison was repeated on
that final build before reporting its results above.

Several plausible changes were discarded: always storing Ruby first,
putting Boarden in Attack Position throughout Main Phase 1, equipping Double
Tool to additional legal holders, preserving direct attackers instead of
Synchro Summoning, changing Rainbow Ruins' effect ordering, and an extra
Giant Trunade veto. They lost points or showed no clear benefit. Additional
Mammoth/Tortoise defensive rules changed no screening outcomes on top of the
combined policy; storing Ruby specifically for Promise also did not help.
Screening intervals are exploratory; the fresh held-out comparison above is
the confirmation.

## Search headroom

Before and after searches used seed `16940000`, eight games per reference
opponent, with the default 8/32/96-world stages, strict mode enabled and
foresight disabled. Search logs exposed missed Release and Abundance plays,
Crystal Tree placements, spare Ruby summons, Malefic summons, Scopen/Celfon
priorities, Cord activations, and Power Tool's position choice. Ruby's mass
summon scoring also came from code and script inspection; search does not
explore card-selection alternatives.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Crystal Beast | before | 96 | 25.0% | 45.8% | 20.8 pp |
| Crystal Beast | after | 96 | 31.2% | 42.7% | 11.5 pp |
| Morphtronic | before | 96 | 32.3% | 50.0% | 17.7 pp |
| Morphtronic | after | 96 | 36.5% | 47.9% | 11.5 pp |

These small discovery samples estimate headroom, not hard ceilings or gains
guaranteed achievable by an ordinary policy. Strict search skips positions
with hidden opposing monsters, but samples from the opponent's real deck
composition and preserves engine random state, including future dice results.
This is particularly relevant to Morphtronic Celfon. Those privileges belong
to development search, not these policies or their paired comparisons.

Both pilots are reference opponents. The follow-up searches therefore also
face the improved other pilot; the paired comparisons keep opponents fixed.
Search is one step on top of the current policy. No search playout failed
and no searched duel hit the decision limit.

## Ranking refresh and validation

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from the complete
119,040-game round robin: all 31 decks, 256 games per pair, seed `730000`.
It had zero failures and zero decision-limit draws.

| Deck | Rank before → after | Measured-pool score before → after |
|---|---:|---:|
| Crystal Beast | #23 → #22 | 33.0% → 40.8% |
| Morphtronic | #22 → #19 | 37.6% → 44.2% |

All **103,936 games involving neither changed policy** reproduced the previous
tournament's outcomes and transcript digests exactly. Rebuilding the tier list
from saved results reproduced its Markdown byte-for-byte.

**105 workspace tests passed**, including ten new regressions for Ruby's
recruitment and normal summon, Tree's counter and zone requirements, Abundance,
Release, Malefic's field requirement, Scopen/Celfon priority, Power Tool's
position, and Cord's setup order, targets, and legal holder.

- [Paired, screening and search results](crystal-morphtronic-2026-10-06.summary.json)
- [Run metadata and raw-result hashes](crystal-morphtronic-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-crystal-morphtronic.summary.json)
- [Ranking metadata](round-robin-2026-10-06-crystal-morphtronic.metadata.json)

## Reproduction

Preserve the baseline shared library at the revision above, then build the
candidate with `cargo build --release -p ygo-policies-ffi -p ygo-policies-bench`.
With `$BASELINE` and `$CANDIDATE` naming the libraries and `$OUT` a fresh directory:

```sh
target/release/policy-bench compare \
  --policies crystal,morphtronic --opponents all --games 512 --seed 19160000 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" --workers 8 \
  --output "$OUT/heldout.jsonl"

# Repeat with each library for before/after search headroom.
target/release/policy-bench search \
  --policies crystal,morphtronic --opponents existing --games 8 --seed 16940000 \
  --library "$CANDIDATE" --workers 8 --strict true --foresight false \
  --worlds 8 --confirm 32 --final 96 --log true --record true \
  --output "$OUT/search.jsonl"

target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 8 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```

Engine, scripts, database and deck identities are recorded in metadata.
Large JSONL logs, recorded positions and frozen libraries remain local in
`/tmp/ygo-crystal-morphtronic.ariqe8pw`; they are not added to Git.
