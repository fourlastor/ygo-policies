# Rules from the search: Blackwing and Monarch

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone is a floor on what a better pilot
of the same deck would win.  This is what came of reading its logs for
rules, for the two decks it was first run on.

## Method

1. A logged search (`--log true`), 64 games against each of the 12 reference
   decks.
2. The decisions where the search left the pilot's answer are grouped by
   what was swapped for what ("Set Battle Fader" for "end the turn"), and the
   positions of a group are read (`seen`) until they say a rule.
3. The rule goes behind a switch and is measured with `policy-bench compare`
   on the same seeds, 512 or 1,024 games against each reference deck.

Only step 3 counts.  A group of overrides is where one answer won clearly
more often *in the positions the search picked*; the same rule also fires in
positions where it loses, and the search's own first-stage numbers lean its
way (decisions the pilot wins outright are not tried at all).

## Result

`policy-bench compare`: each deck against the 30 others, 512 games per
pairing, the policies of this commit against the ones before it.

| Deck | Before | Now | Change [95% CI] | Games with a different result |
|---|---:|---:|---:|---:|
| blackwing | 65.3% | 71.0% | +5.7 [+5.1, +6.3] | 2,127 of 15,360 |
| monarch | 71.2% | 76.1% | +4.9 [+4.4, +5.5] | 2,073 of 15,360 |

The search against the pilots, before and after the first round of rules
(against the 12 reference decks):

| Pilot | Games | Alone | With search | Gap |
|---|---:|---:|---:|---:|
| Blackwing, before | 576 | 62.0% | 78.8% | 16.8 |
| Blackwing, after the first round | 768 | 68.9% | 79.9% | 11.0 |
| Monarch, before | 768 | 74.2% | 88.7% | 14.5 |
| Monarch, after the first round | 768 | 78.3% | 89.3% | 11.0 |

With the search the decks win about what they won before: the rules moved
the pilots toward it, not it further up.  No search was run on the pilots of
this commit.

## The rules that held

Each with its own paired gain when it was added, against the 12 reference
decks (6,144 to 12,288 games).

Blackwing:

- Sirocco, Shura, Bora and Zephyros (1600 ATK and more) are never Set:
  face-down they hold nothing, and Black Whirlwind searches nothing (+2.8);
- a Blackwing Special Summoned in Main Phase 1 stands in Attack Position,
  and Sirocco's effect counts every face-up Blackwing (+1.3);
- Sirocco is Tribute Summoned when its effect then takes one attacker over a
  wall nobody gets through (+1.2);
- Pot of Duality is no longer kept back in Main Phase 1 (+0.6);
- Mystical Space Typhoon takes a face-up Spell or Trap their deck runs on
  at the first chance, not at the end of their turn (+0.5);
- Sirocco comes down without a Tribute before a Spell fills our field (+0.4);
- a Set Icarus Attack, Threatening Roar, Typhoon or Book of Moon that their
  Typhoon or Heavy Storm is about to destroy is chained first (+0.2);
- Dark Hole is not played for one monster this turn's Normal Summon beats
  (about +0.1).

Monarch:

- Soul Exchange takes their monster as the Tribute even with one of our own:
  the Monarch's effect then removes a second card (+1.3);
- Battle Fader is never Set (+1.1);
- a Set Gravekeeper's Spy is Flip Summoned before the Monarch takes its
  Tribute: its effect brings a second Spy (+0.8);
- Treeborn Frog's revival is declined when it would keep Pot of Duality in
  hand, unless a Monarch waits for the Tribute (+0.75);
- with no Monarch in hand, Swap Frog returns the one on the field, and it is
  Tribute Summoned again for its effect (+0.4);
- Jester Confit is Special Summoned on its own while they control a face-up
  monster: it takes one back to the hand with it (+0.3 to +0.6);
- Dark Hole as for Blackwing (+0.4);
- with only Monarchs in hand and no monster of ours, One for One and Soul
  Exchange are used: no Summon is offered yet, and the rule that asked for
  one never let them (+0.2).

Two of these the search could not have shown.  It skips every decision
taken inside a chain, where the Set cards are saved; and it tries an effect
with the target the pilot would pick, so Swap Frog returning the Monarch came
from asking what its overrides on Swap Frog were for.

## What did not hold

The first round turned most of what the logs showed into rules.  In the
second round, on the improved pilots, four candidates of fourteen held.
Paired, 12,288 games each:

| Candidate | Change [95% CI] |
|---|---:|
| Blackwing: Bora and Gale by their own Special Summon, the Normal Summon for another Blackwing | -0.34 [-0.76, +0.08] |
| the same, only when the Special Summon is on offer | -0.29 [-0.51, -0.07] |
| Blackwing: Icarus Attack on their turn before a lone Blackwing is lost (four forms) | -0.26 to +0.10 |
| Blackwing: no Blackwing Set while Icarus Attack needs its Tribute | -0.02 [-0.14, +0.11] |
| Blackwing: Sirocco's Tribute Summon counts Bora and Gale in hand | -0.02 [-0.11, +0.06] |
| Blackwing: Threatening Roar waits while Dimensional Prison is Set | +0.01 [-0.14, +0.15] |
| Blackwing: Zephyros returns from the Graveyard more freely (three forms) | -0.01 to +0.09 |
| Blackwing: no Dark Hole for one monster (three Life Point bounds) | -0.47 to +0.12 |
| Monarch: Thestalos first when nothing of theirs needs an effect to go | +0.06 [-0.25, +0.36] |
| Monarch: Typhoon before its own fodder and Soul Exchange | -0.24 [-0.41, -0.08] |
| Monarch: Treeborn Frog stays down while Gorz needs an empty field | -0.05 [-0.21, +0.11] |

The first line is the clearest case.  In the log, Normal Summoning another
Blackwing and following with Bora won in single positions with z up to 4.7.
As a rule it puts one monster more on the field every time it can, and Heroes
(-2.4), Monarch (-1.6) and Lightsworn (-1.2) punish that.  What the search
still has over these pilots is mostly of this kind: right in the position,
wrong as a habit.

From the first round, also measured and dropped: for Blackwing, Setting on
turn 1, Sirocco's Tribute Summon for Black Whirlwind's search, Monster Reborn
after the Normal Summon; for Monarch, Swap Frog first to send Treeborn Frog
to the Graveyard (-1.0), One for One without a Monarch in hand (-0.3), Battle
Fader only against big attacks, Treeborn Frog declined whenever Pot of
Duality is in hand.

## Records

- `search-rules-vs-old.*`: the comparison of the result table (seeds,
  libraries, card database and scripts in the metadata).
- Unit tests: `blackwing_plays_what_the_search_found`,
  `monarch_plays_what_the_search_found`,
  `blackwing_uses_a_set_card_before_it_is_destroyed`.
- The logged searches are about 25 MB each and are not kept here.
  `policy-bench search --policies blackwing --opponents existing --games 64
  --log true`, with the `--core` build the search needs, writes them again
  on the commit before this one.
