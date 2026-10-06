# Harpie Sisters: legal deck and policy improvements

Baseline: `c87341547c94903230215ac37c66d48f14271e34`. Work is on `codex/harpie-deck-policy`.
Only the Harpie list and policy change. The 31-deck ranking pool, other
policies, engine, card database, and scripts remain the same.

## A legal WC2011 deck

The previous list contained seven cards that always count as Harpie Lady.
[Konami's official FAQ](https://www.db.yugioh-card.com/yugiohdb/faq_search.action?ope=4&cid=6186&request_locale=ja)
confirms that Harpie Lady, Cyber Harpie Lady, and Harpie Lady 1/2/3 share a
three-copy deck-construction limit. Queen changes name only on the field/in
the Graveyard and has a separate limit.

The replacement has **40 Main Deck cards**, no Extra or Side Deck, and
**two Harpie Lady 1 plus one Cyber Harpie Lady**, alongside three Queens.
Every card belongs to `data/wc2011.lflist.conf`; database aliases and the
whitelist's copy limits were checked. A regression test guards the pool,
individual limits, and the combined Harpie Lady count.

| Removed | Copies | Added | Copies |
|---|---:|---|---:|
| Harpie Lady 2 | 1 | Harpie's Pet Baby Dragon | 1 |
| Harpie Lady 3 | 1 | Birdface | 1 |
| Harpie Lady | 2 | Mist Valley Falcon | 1 |
| Harpie Girl | 2 | Hysteric Party | 1 |
| Harpie's Pet Dragon | 1 | Bottomless Trap Hole | 1 |
| Double Summon | 1 | Dimensional Prison | 2 |
|  |  | Hunter Owl | 1 |

Sisters, Egotist, Triangle Ecstasy Spark, Hunting Ground, and Hysteric Party
remain the deck's core. The final list has 17 monsters, with Falcon and
Birdface replacing illegal Lady copies, Hunter Owl replacing the tribute
Pet Dragon, and more defensive traps replacing Harpie Girl and Double Summon.

## Separating the deck and policy gains

The deck was selected first using the frozen old policy. Policy rules were
then screened on that fixed legal list. Neither stage used the held-out
scores for tuning. Held-out seed `33590000` covers 512 games against each
of the 30 other decks, **15,360 games per version**. Each stage uses matching
seeds and seats and unchanged opposing decks/policies. Changing card lists
changes draws; these are paired outcomes, not identical decision sequences.
Draws count as half a win. Intervals below are paired normal 95% intervals.

| Change | Before | After | Gain, pp [95% CI] |
|---|---:|---:|---:|
| Deck only, old policy | 46.76% | 57.60% | +10.84 [+9.97, +11.71] |
| Policy only, legal deck | 57.60% | 61.58% | +3.98 [+3.40, +4.55] |
| Combined, versus original illegal deck | 46.76% | 61.58% | +14.82 [+13.95, +15.69] |

All three held-out runs completed without failures or decision-limit draws.
Deck change: 27 positive matchup estimates, 0 ties, 3 negative.
Policy change: 30 positive matchup estimates, 0 ties, 0 negative.
Combined change: 28 positive matchup estimates, 0 ties, 2 negative.

The combined change has two negative matchup estimates (512 pairs each),
reported without further tuning:

- burn: 90.62% → 89.84%, -0.78 [-3.89, +2.33] pp.
- countdown: 10.35% → 5.27%, -5.08 [-7.96, -2.19] pp.

## Policy rules

- Wait to activate Hunting Ground or discard Queen to search it until the
  opponent has backrow. This avoids spending Queen or immediately triggering
  Hunting Ground's mandatory destruction on our own cards. A previously set
  Hunting Ground can also be activated when an opposing target appears.
- Use Hysteric Party in Main Phase 1 even for one Lady. In other response
  windows, revive multiple Ladies, or one Lady when our monster field is empty.
  The engine still determines legal activations and revival targets.
- When Falcon pays its attack cost, return set backrow before a Lady that
  can contribute to the Battle Phase. Continue protecting face-up Party.
- If Hunting Ground must destroy our own backrow, prefer sacrificing the
  field spell instead of live support, especially a Party sustaining monsters.

All rules read permitted observations, public printed data, and legal choices.
They do not inspect opposing hidden identities, deck order, engine snapshots,
or future random results.

## Screening and development

Deck screens used seed `30260000`, 256 games against each of 13 reference
decks: 3,328 games per candidate, with the same frozen policy. The minimal
legal replacement scored 39.09%, the original illegal list 41.59%, and the
selected replacement 52.07%. The selected combination retains the individually
promising swaps. More complicated combinations did not establish a clear
advantage over it in this exploratory screen. Detailed variant compositions
and all screen results are in the compact evidence.

Policy screens used seed `32480000`, 256 games per reference opponent:
3,328 paired games per rule or combination, on the selected legal list.

| Candidate | Gain, pp [95% CI] |
|---|---:|
| Combined retained rules | +5.47 [+4.16, +6.78] |
| Delay Duality for a possible Egotist summon | +0.18 [-0.07, +0.43] |
| Prefer Lady 1 over Sisters for Egotist | +0.21 [-0.56, +0.98] |
| Falcon returns set backrow before attackers | +1.71 [+1.11, +2.32] |
| Ground waits for opposing backrow, including activation from set | +2.76 [+1.71, +3.82] |
| Sacrifice Ground before other own backrow | +0.33 [+0.07, +0.59] |
| Ground waits for opposing backrow (from hand only) | +2.76 [+1.71, +3.82] |
| Proactive Icarus in Main Phase | -1.11 [-1.96, -0.26] |
| Icarus against two or more opposing monsters | +0.18 [-0.62, +0.98] |
| Icarus when opposing ATK exceeds ours | +0.36 [-0.40, +1.12] |
| Icarus at every two-target opportunity | -1.02 [-2.05, +0.00] |
| Raise Lady 1 summon priority with other WIND monsters | +0.03 [-0.25, +0.31] |
| Raise Hunter Owl summon priority with WIND allies | +0.45 [-0.14, +1.04] |
| Party without the timing guard | +0.81 [-0.06, +1.68] |
| Party for attacking, rebuilding, or multiple revivals | +0.96 [+0.14, +1.78] |
| Party in Main Phase with two Ladies in GY | +0.24 [-0.20, +0.68] |
| One Lady as an emergency blocker | +0.48 [+0.06, +0.90] |
| Queen searches only with another body available | +0.93 [+0.44, +1.42] |
| Queen searches only with opposing backrow | +2.52 [+1.63, +3.41] |

The final larger development run used 1,024 games per reference opponent,
**13,312 pairs**, and improved from **52.64% to 57.13%**:
**+4.49 [+3.82, +5.17] pp**. All 3,328 selected screening pairs reproduced their scores
and decision digests after code cleanup. Screening results guided selection;
the held-out sample is the independent confirmation.

Unconditional Icarus use, extra summon priorities, Egotist targeting changes,
and a Duality veto were tested and omitted. Icarus retains its defensive role.

## Search headroom on the legal list

Both searches used seed `31370000`, eight games per reference opponent,
104 games total, 8/32/96-world stages, strict mode enabled, and foresight
disabled. Opponent policies and deck lists are identical in both searches.

| Policy version | Alone | With search | Gap |
|---|---:|---:|---:|
| Before | 50.96% | 71.15% | 20.19 pp |
| After | 57.69% | 70.19% | 12.50 pp |

The discovery logs repeatedly favored Party activation, saving Queen for a
summon, and different Hunting Ground timing. Falcon's selection rule and
own-field targeting were developed from the policy and pinned scripts;
search does not explore card-selection alternatives. These small searches
estimate headroom, not hard ceilings. Their hidden-deck-composition and engine
random-state access remains confined to search, never the ordinary policy.
Both searches completed without failed playouts or decision-limit draws.
An ordinary paired comparison on all 104 search seeds reproduced both
standalone versions' scores and decision digests exactly.

## Canonical ranking and checks

The full all-31 round robin used 256 games per pair, seed `730000`:
**119,040 games**, zero failures and zero decision-limit draws.
Harpie moved from **#20 (46.9%, illegal list)** to
**#10 (60.8%, legal list and improved policy)**.
All **111,360 games not involving Harpie** reproduced their prior scores and
decision digests exactly. The [tier list](../DECK-TIER-LIST.md) was regenerated
and independently reproduced byte-for-byte from the saved results.

The workspace passed **141 tests**, including the new deck-legality guard
and regressions for Hunting Ground, Queen, Party, and Falcon decisions.

- [Compact measurements](harpie-2026-10-06.summary.json)
- [Arguments, deck variants, fingerprints, and source/result hashes](harpie-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-harpie.summary.json)
- [Ranking metadata](round-robin-2026-10-06-harpie.metadata.json)

Raw JSONL games, traces, and frozen libraries remain outside Git in `/tmp/ygo-harpie.agx95xg0`.
