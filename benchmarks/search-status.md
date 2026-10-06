# The search, deck by deck

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone is a floor on what a better pilot
of the same deck would win, and its logged games say where to look for rules
([method](search-rules.md#method)).

Nine pilots have been through it.  The other 22 are to follow, a few at a
time: a logged search of about 400 games takes 7 to 25 minutes on 22 workers,
and reading it and measuring each rule it suggests takes a few hours more.

## Done

The first five pilots were searched against 12 reference decks; the latest
four use the current 13. The first five entries show searches before their
latest rules; Ojama, Destiny HERO, Watt and Toon show the follow-up search
after their rules, 104 games each. Search gaps are estimates of remaining headroom,
not hard ceilings.

| Deck | Searches | Last search: alone | With the search | Gap | Win rate before → after its rules | Record |
|---|---:|---:|---:|---:|---|---|
| Infernity | 2 | 59.1% | 76.6% | 17.5 | 52.6% → 62.5%, then 62.5% → 65.2% (reference decks) | [game-run-rules.md](game-run-rules.md#infernity) |
| Fortune Lady | 1 | 34.5% | 54.0% | 19.5 | 34.1% → 41.5% (reference decks) | [game-run-rules.md](game-run-rules.md#fortune-lady) |
| Draconic Might | 1 | 58.2% | 71.6% | 13.4 | 58.9% → 59.8% (reference decks) | [game-run-rules.md](game-run-rules.md#draconic-might) |
| Blackwing | 2 | 68.9% | 79.9% | 11.0 | 65.3% → 71.0% (the 30 other decks) | [search-rules.md](search-rules.md) |
| Monarch | 2 | 78.3% | 89.3% | 11.0 | 71.2% → 76.1% (the 30 other decks) | [search-rules.md](search-rules.md) |
| Ojama Brigade | 2 | 18.3% | 27.9% | 9.6 | 14.63% → 17.01% (the 30 other decks, held out) | [ojama-destiny-rules.md](ojama-destiny-rules.md) |
| Destiny HEROes | 2 | 18.3% | 30.8% | 12.5 | 17.61% → 19.07% (the 30 other decks, held out) | [ojama-destiny-rules.md](ojama-destiny-rules.md) |
| Watt Grid | 3 | 30.8% | 45.2% | 14.4 | 17.52% → 20.49%, then 20.96% → 28.18% (the 30 other decks, held out) | [watt-followup-rules.md](watt-followup-rules.md) |
| Toon Kingdom | 2 | 21.2% | 30.8% | 9.6 | 20.65% → 24.07% (the 30 other decks, held out) | [watt-toon-rules.md](watt-toon-rules.md) |

A deck is not finished after one search: a second one on the improved
Infernity pilot led to 2.7 points more.  Fortune Lady has the widest gap
left and has only been searched once.

## To do

The 22 other decks, in the order of the [current tier list](../DECK-TIER-LIST.md)
refreshed on 2026-10-06. Crystal Beasts and Gishki were in the recorded run
of the game: their reported mistakes are checked
([game-run-rules.md](game-run-rules.md)), a search is not run yet.
Harpie remains in the ranking pool, but its illegal list must be replaced
and remeasured before policy work.

| # | Deck | Policy | Win rate | Tier | |
|---:|---|---|---:|---|---|
| 1 | Claudi-oh's Countdown | `countdown` | 93.8% | strong |  |
| 2 | Claudi-oh's Verdict | `verdict` | 83.8% | strong |  |
| 4 | Machina Gadgets | `machina` | 72.5% | strong |  |
| 5 | Gravekeeper's Tomb | `gravekeeper` | 69.5% | strong |  |
| 8 | Fusion Heroes | `heroes` | 63.7% | mid |  |
| 9 | Legendary Six Samurai | `six-samurai` | 63.5% | mid |  |
| 11 | Lightsworn Judgment | `lightsworn` | 59.6% | mid |  |
| 12 | Tele-DAD | `tele-dad` | 59.2% | mid |  |
| 13 | 06 Koaki Meiru - Rock Block | `rock-block` | 58.6% | mid |  |
| 14 | Fight, Gladiators! | `gladiator` | 57.3% | mid |  |
| 15 | Dragunity Flight | `dragunity` | 56.2% | mid |  |
| 16 | X-Sabers | `x-saber` | 53.9% | mid |  |
| 17 | Quickdraw Plants | `quickdraw-plant` | 53.1% | mid |  |
| 18 | Harpie Sisters | `harpie` | 49.9% | mid | illegal list; defer policy work |
| 19 | Karakuri Workshop | `karakuri` | 44.8% | weak |  |
| 21 | Undersea Ceremony | `gishki` | 41.4% | weak | in the recorded run |
| 22 | 01 Morphtronic - Straight Up | `morphtronic` | 38.4% | weak |  |
| 23 | 12 Crystal Beast - Rainbow | `crystal` | 33.8% | weak | in the recorded run |
| 24 | Pyramid of Light | `pyramid` | 28.0% | weak |  |
| 26 | 14 - Spellcaster's Command | `spellcaster` | 26.0% | weak |  |
| 27 | Burn Princess | `burn` | 24.7% | weak |  |
| 29 | Arcana Force Fortune | `arcana` | 21.2% | weak |  |
