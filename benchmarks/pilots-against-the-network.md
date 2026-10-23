# The pilots against the network

A pass over the pilots of the decks a network plays best beside them. The
network is `all71` of the training repository (one network for all 71 decks,
its run's last checkpoint): after imitating these pilots and its PPO, it plays
56 of the 58 decks it was measured on better than their own pilots. It is a
diagnostic only. No pilot calls it, and nothing of it becomes a pilot's input;
the pilots still see only their seat's observation and legal choices. Lists,
engine, card scripts and card database are unchanged.

## Method

Where a deck's gap sits comes from the training repository's census
(`tools/pilot_census.py`): the deck's pilot is asked alongside the network's
seat, and the network answers the transactions of some kinds of decision
(main-phase commands, chain windows, selections, ...) while the pilot answers
the rest, on the same deals. Within a deck, smaller classes of disagreement
(the pilot's answer and the network's, by card and action) are measured the
same way before a rule is written. Classes were often worth far more together
than apart: a summon and the fusion it feeds are one plan.

Each rule is a `compare` of the library before it and after it, the deck
against the thirteen existing pilots, 1,040 pairs (80 an opponent) from seed
970000, and is kept when it wins more by two standard errors (a paired 95%
half-width under about 1.96 standard errors). The held-out reading compares
the deck's final pilot with its pilot at `21d49d3` against the other 57
decks' pilots, from seed 990000, 40 pairs an opponent; it holds when the
final pilot wins at least 2 points more by two standard errors.

## Cyber (Power Surge)

| Rule | Before | After | Change, paired 95% | Kept |
|---|---:|---:|---:|---|
| Chimeratech Overdragon and Fortress Dragon summoned in Attack | 20.87% | 28.56% | +7.69 ± 1.90 | yes |
| Chimeratech Fortress Dragon only of the opponent's Machines | 28.56% | 32.31% | +3.75 ± 1.74 | yes |
| Cyber Dragon special summoned in Attack | 32.31% | 32.21% | −0.10 ± 0.42 | no |
| Cyber Phoenix summoned face-up, never set | 32.31% | 32.12% | −0.19 ± 1.31 | no |
| Zwei, Proto and Phoenix never set | 32.31% | 32.60% | +0.29 ± 1.54 | no |
| Zwei or Proto summoned before Power Bond or Polymerization | 32.31% | 34.90% | +2.60 ± 1.58 | yes |
| Overload Fusion banishes every Machine of the Graveyard it may | 34.90% | 42.12% | +7.21 ± 1.68 | yes |
| Polymerization only before a Battle Phase | 42.12% | 42.02% | −0.10 ± 0.94 | no |
| Future Fusion brings Cyber Twin Dragon where it can | 42.12% | 43.85% | +1.73 ± 1.06 | yes |
| No Dark Hole while our monster can fuse | 43.85% | 44.04% | +0.19 ± 0.60 | no |

Why they pay:

- The Chimeratech fusions' printed ATK is `?`, which the shared position
  rule read as 0 against the opponent's best attacker: they went to Defense,
  where Fortress Dragon has 0 DEF.
- Fortress Dragon from our own field traded Cyber Dragon and another
  attacker for one monster no stronger.
- Face-up on the field Zwei and Proto are Cyber Dragon: summoned first, the
  fusion is Cyber Twin Dragon rather than a small Overdragon under Power Bond.
- Fusion materials are a cost to the shared toggle selection, which stops at
  the minimum; Overdragon has 800 ATK a material
  (`Strategy::continue_optional_selection`).
- Future Fusion's Twin sends two Cyber Dragons and keeps the Deck's other
  Machines.

Held out, against the other 57 decks' pilots: **22.89% → 45.00%, +22.11
points, paired 95% half-width 1.94**, 2,280 pairs, 489 identical, 563
improved, 59 regressed.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 21.8%, the
final pilot 46.8%, the network 57.4%; the final pilot closes 70% ± 4% of the
network's gap.

Head to head in the Cyber mirror against the network (1,040 games, 520 seeds
from 985000, each both ways): the pilot at `21d49d3` wins 24.9% ± 1.3%, the
final pilot 44.3% ± 1.5%.

## Zombie (Graveyard Shift)

Rules over the library before them, Zombie against the thirteen, 1,040 pairs
from seed 970000 unless said. Related rules were measured together as well as
apart before any was dropped.

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| A: Il Blud Normal Summoned from the hand for a Tribute (the deck had vetoed it) | `21d49d3` | +3.37 ± 1.99 | yes |
| B: face-down Pyramid Turtle and Spirit Reaper flipped in Main Phase 1 before a Battle Phase | A (2,080 pairs) | +1.44 ± 1.17 | yes |
| F: Card Trooper Normal Summoned when a Battle Phase follows and their best attacker has 1200 ATK or less | A, B | +2.50 ± 1.64 | yes |
| E: Ryko, Lightsworn Hunter the first Set (alone over A, B: +1.92 ± 2.03) | A, B, F | +2.79 ± 2.07 | yes |
| C: a face-up attacker weaker than their best to Defense outside Main Phase 1 (alone over A: +0.48 ± 0.68) | A, B, E, F | +0.58 ± 0.46 | yes |
| D: Pyramid Turtle Normal Summoned against small monsters | A, B, E, F; with C | +0.48 ± 1.54; +0.67 ± 1.61 | no |
| G: Dark Hole only against two monsters or more | all kept | −0.38 ± 1.22 | no |
| H: Torrential Tribute only against two monsters or more | all kept | −1.15 ± 0.84 | no |
| G and H together | all kept | −2.12 ± 1.48 | no |
| I: Plaguespreader revived only against a monster | all kept | −0.10 ± 0.94 | no |
| Plaguespreader never revived from the Main Phase | all kept | +0.29 ± 1.68 | no |
| J: Stardust Dragon and Revived King Ha Des over the other Synchros | all kept | −0.38 ± 1.44 | no |

Ryko set first pays only with Card Trooper summoned face-up: both send cards
from the Deck to the Graveyard the revivals draw on.

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**52.94% → 58.55%, +5.61 points, paired 95% half-width 1.87**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 49.6%, the
final pilot 56.0%, the network 76.3%; 24% ± 5% of the network's gap closed.
What is left lies in when to fire removal and revivals, which the network
judges position by position: holding them where the pilot fires them is worth
some 9 points to the network, but plain holds lose.

Head to head in the Zombie mirror against the network (1,040 games, 520 seeds
from 985000, both ways): the pilot at `21d49d3` wins 13.6% ± 1.0%, the final
pilot 17.8% ± 1.2%.

## Chain Burn (Chain Reaction)

The network's gap was almost all in chain windows. It fired the draw and burn
cards about as often as the pilot, but at other moments: rarely to open a
chain or onto a single link (1–25% of the windows offered), often onto chains
of two links or more (45–95%), in Main Phase 1 and at the Battle Phase rather
than at the turn's start; and it played Accumulated Fortune five times as
often (0.54 a game against 0.11).

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| K: with Fortune Set, a chain opened only with three different chain cards ready | `21d49d3` | +3.37 ± 1.51 | yes |
| K, whether Fortune is Set or not | `21d49d3` | +3.08 ± 1.74 | (K kept) |
| N: Poison of the Old Man Set, not played alone from the main phase | K | +0.67 ± 0.62 | yes |
| P: Poison chained only from the field | K, N | +2.31 ± 1.18 | yes |
| S: Swift Scarecrow only for attackers worth half our Life Points (alone +1.44 ± 1.01) | K, N, P | +1.83 ± 1.04 | yes |
| T: no draw or burn cards in the Draw and Standby Phases | the above | +3.46 ± 1.84 | yes |
| U: with Fortune Set, any chain joined only when Fortune can come fourth | the above (2,080 pairs) | +2.00 ± 1.00 | yes |
| U, whether Fortune is Set or not | the above | −7.60 ± 2.36 | no |
| Fortune, then Chain Strike, Set first | the above (2,080 pairs) | +0.19 ± 0.63 | no |
| Waboku only for attackers worth half our Life Points | the above (2,080 pairs) | +1.44 ± 0.84 | yes |
| Threatening Roar only for a quarter (alone; with Waboku) | the above | +0.10 ± 0.57; +1.63 ± 1.34 | no |

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**31.49% → 48.99%, +17.50 points, paired 95% half-width 1.87**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 22.8%, the
final pilot 41.2%, the network 56.0%; 55% ± 4% of the network's gap closed.

Head to head in the Chain Burn mirror against the network (1,040 games, 520
seeds from 985000, both ways) the final pilot does worse: 43.9% ± 1.1%
before, 39.4% ± 1.1% after. Its gains are against pilots; against the
network's own chains, holding cards for longer ones loses.

## Herald (Herald's Veto)

The network attacked face-down monsters with Dunames Dark Witch, Manju and
Herald of Perfection where the pilot went to Main Phase 2: the shared rule
attacks an unknown face-down monster only from 1900 ATK, which none of the
deck's attackers has. Given the network's answers there alone it won 8.5
points more.

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| Attack unknown face-down monsters from 1800 ATK | `21d49d3` | +8.56 ± 1.88 | (1400 kept) |
| Attack unknown face-down monsters from 1400 ATK | `21d49d3` | +9.04 ± 1.95 | yes |

What is left of the gap is spread over small pieces, none worth more than
2.3 points to the network alone: Herald of Perfection's negations of small
plays (Crystal Promise, Gladiator Proving Ground, Treeborn Frog), Summons the
pilot keeps in hand as Fairy fuel, Royal Decree and Beckoning Light in chain
windows.

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**39.61% → 50.04%, +10.44 points, paired 95% half-width 1.67**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 30.9%, the
final pilot 39.5%, the network 55.5%; 35% ± 4% of the network's gap closed.

Head to head in the Herald mirror against the network (1,040 games, 520
seeds from 985000, both ways): 28.5% ± 1.3% before, 42.0% ± 1.2% after.

## Iron Chain (Rust Never Sleeps)

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| E: Ryko, Lightsworn Hunter the first Set | `21d49d3` | +6.73 ± 2.42 | yes |
| F: Card Trooper Normal Summoned when a Battle Phase follows and their best attacker has 1200 ATK or less (alone +2.12 ± 1.57) | E (2,080 pairs) | +1.68 ± 1.16 | yes |
| G: Debris Dragon Normal Summoned when a monster of 500 ATK or less waits in the Graveyard, never Set (alone, 2,080 pairs) | E, F | +0.91 ± 1.22 | with H |
| H: Dandylion an early Set (alone) | E, F | +0.38 ± 1.53 | with G |
| G and H together: 1,040 pairs; 2,080; 1,040 fresh pairs from seed 975000 | E, F | +1.83 ± 2.28; +1.68 ± 1.54; +2.21 ± 2.23 | yes |
| Poison Chain not played from the main phase; Iron Chain Coil not used there; both | E, F | +0.19 ± 1.19; −2.60 ± 1.23; −2.31 ± 1.59 | no |
| Dark Hole only against two monsters or more with none of ours: 1,040; 2,080; 1,040 fresh | E, F, G, H | +1.25 ± 1.74; +1.39 ± 1.16; +0.19 ± 1.60 | no |
| Dark Hole only against two monsters or more | E, F, G, H | +1.15 ± 1.43 | no |

Debris Dragon and Dandylion pay only together: Debris brings Dandylion back.
Their pass was reached on 2,080 pairs after 1,040 did not pass; fresh pairs
agreed, and all 3,120 together give about +1.9 with a standard error of 0.65.
The first Dark Hole rule passed on 2,080 pairs the same way, but fresh pairs
did not confirm it, so it was dropped.

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**44.21% → 52.54%, +8.33 points, paired 95% half-width 2.01**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 35.9%, the
final pilot 47.2%, the network 64.8%; 39% ± 5% of the network's gap closed.

Head to head in the Iron Chain mirror against the network (1,040 games, 520
seeds from 985000, both ways): 18.9% ± 1.1% before, 30.9% ± 1.3% after.

## Amazoness (Queens of the Wild)

The network played the deck in Attack: it Normal Summoned what the pilot Set,
Special Summoned in Attack what the pilot put in Defense, and flipped its
monsters to Attack before battle. Village, Fighting Spirit and Swords Woman
all pay in Attack.

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| A2: Amazoness never Set, Normal Summoned face-up | `21d49d3` | +5.48 ± 2.58 | yes |
| A1: Amazoness Special Summoned in Attack (alone +4.04 ± 1.51) | A2 | +1.92 ± 1.22 | yes |
| A3: Amazoness switched to Attack before the Battle Phase (alone +2.40 ± 1.58) | A1, A2 | +0.00 ± 0.27 | no |
| Paladin over Sage in searches: 1,040; 2,080; 1,040 fresh from seed 975000 | A1, A2 | +1.54 ± 1.60; +1.44 ± 1.11; +0.38 ± 1.51 | no |

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**57.32% → 61.67%, +4.34 points, paired 95% half-width 1.46**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 49.1%, the
final pilot 54.6%, the network 67.5%; 30% ± 6% of the network's gap closed.

Head to head in the Amazoness mirror against the network (1,040 games, 520
seeds from 985000, both ways): 29.0% ± 1.2% before, 36.5% ± 1.2% after.

## Malefic (Eclipse Without End)

The opponent's Field Spell counted as support for the Malefics, so the pilot
kept its own in hand: with no Field Spell of its own the network played
Malefic World 79% of the times it could (the pilot 47%) and Terraforming 80%
(the pilot 21%). It also Set Malefic Parallel Gear, which the pilot never did
outside one combo.

| Rule | Measured over | Change, paired 95% | Kept |
|---|---|---:|---|
| Our own Field Spell, whatever the opponent's | `21d49d3` | +2.79 ± 1.34 | yes |
| Malefic Parallel Gear may be Set | the above | +2.40 ± 1.49 | yes |
| Terraforming for Malefic World whenever it is not in hand or ours: 1,040; 2,080; 1,040 fresh from seed 975000 | the above | +1.15 ± 1.25; +1.06 ± 0.92; +1.92 ± 1.27 | yes |

Held out, against the other 57 decks' pilots from seed 990000 (2,280 pairs):
**57.94% → 64.56%, +6.62 points, paired 95% half-width 1.47**.

Beside it, in the training repository's census on new deals (1,040, seeds
from 980000, against the thirteen): the pilot at `21d49d3` wins 59.7%, the
final pilot 65.6%, the network 71.7%; 49% ± 9% of the network's gap closed.

Head to head in the Malefic mirror against the network (1,040 games, 520
seeds from 985000, both ways): 34.6% ± 1.3% before, 46.8% ± 1.3% after.
