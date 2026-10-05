# The search, deck by deck

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone is a floor on what a better pilot
of the same deck would win, and its logged games say where to look for rules
([method](search-rules.md#method)).

Five pilots have been through it.  The other 26 are to follow, a few at a
time: a logged search of about 400 games takes 7 to 25 minutes on 22 workers,
and reading it and measuring each rule it suggests takes a few hours more.

## Done

The searches play against the 12 reference decks.  The last search of each
deck was run before the rules it gave, so its gap is the one those rules
started from.

| Deck | Searches | Last search: alone | With the search | Gap | Win rate before → after its rules | Record |
|---|---:|---:|---:|---:|---|---|
| Infernity | 2 | 59.1% | 76.6% | 17.5 | 52.6% → 62.5%, then 62.5% → 65.2% (reference decks) | [game-run-rules.md](game-run-rules.md#infernity) |
| Fortune Lady | 1 | 34.5% | 54.0% | 19.5 | 34.1% → 41.5% (reference decks) | [game-run-rules.md](game-run-rules.md#fortune-lady) |
| Draconic Might | 1 | 58.2% | 71.6% | 13.4 | 58.9% → 59.8% (reference decks) | [game-run-rules.md](game-run-rules.md#draconic-might) |
| Blackwing | 2 | 68.9% | 79.9% | 11.0 | 65.3% → 71.0% (the 30 other decks) | [search-rules.md](search-rules.md) |
| Monarch | 2 | 78.3% | 89.3% | 11.0 | 71.2% → 76.1% (the 30 other decks) | [search-rules.md](search-rules.md) |

A deck is not finished after one search: a second one on the improved
Infernity pilot led to 2.7 points more.  Fortune Lady has the widest gap
left and has only been searched once.

## To do

The 26 other decks, in the order of the [tier list](../DECK-TIER-LIST.md) of
2026-10-06 (win rate against all 30 other decks).  Crystal Beasts, Destiny
HERO and Gishki were in the recorded run of the game: their reported
mistakes are checked ([game-run-rules.md](game-run-rules.md)), a search is not
run yet.

| # | Deck | Policy | Win rate | Tier | |
|---:|---|---|---:|---|---|
| 1 | Claudi-oh's Countdown | `countdown` | 93.9% | strong |  |
| 2 | Claudi-oh's Verdict | `verdict` | 84.2% | strong |  |
| 4 | Machina Gadgets | `machina` | 73.2% | strong |  |
| 5 | Gravekeeper's Tomb | `gravekeeper` | 69.6% | strong |  |
| 8 | Legendary Six Samurai | `six-samurai` | 64.3% | mid |  |
| 9 | Fusion Heroes | `heroes` | 64.1% | mid |  |
| 11 | Lightsworn Judgment | `lightsworn` | 60.1% | mid |  |
| 12 | Tele-DAD | `tele-dad` | 59.7% | mid |  |
| 13 | 06 Koaki Meiru - Rock Block | `rock-block` | 59.4% | mid |  |
| 14 | Fight, Gladiators! | `gladiator` | 57.6% | mid |  |
| 15 | Dragunity Flight | `dragunity` | 56.6% | mid |  |
| 16 | X-Sabers | `x-saber` | 54.3% | mid |  |
| 17 | Quickdraw Plants | `quickdraw-plant` | 53.6% | mid |  |
| 18 | Harpie Sisters | `harpie` | 50.3% | mid |  |
| 19 | Karakuri Workshop | `karakuri` | 45.7% | weak |  |
| 21 | Undersea Ceremony | `gishki` | 42.2% | weak | in the recorded run |
| 22 | 01 Morphtronic - Straight Up | `morphtronic` | 38.8% | weak |  |
| 23 | 12 Crystal Beast - Rainbow | `crystal` | 35.1% | weak | in the recorded run |
| 24 | Pyramid of Light | `pyramid` | 28.8% | weak |  |
| 25 | 14 - Spellcaster's Command | `spellcaster` | 26.7% | weak |  |
| 26 | Burn Princess | `burn` | 25.1% | weak |  |
| 27 | Arcana Force Fortune | `arcana` | 22.4% | weak |  |
| 28 | Toon Kingdom | `toon` | 20.7% | weak |  |
| 29 | Watt Grid | `watt` | 18.6% | weak |  |
| 30 | Destiny HEROes | `destiny-hero` | 18.3% | weak | in the recorded run |
| 31 | Ojama Brigade | `ojama` | 15.5% | weak |  |
