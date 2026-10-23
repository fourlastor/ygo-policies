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
