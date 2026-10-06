# The search, deck by deck

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone helps identify possible improvements,
and its logged games say where to look for rules
([method](search-rules.md#method)).
Search has privileged deck-composition and random-state access, so its gap is
not a guaranteed gain achievable by an ordinary policy. The policies themselves
receive only information their player is permitted to know.

Seventeen pilots have been through it. The other 14 are to follow, a few at a
time: a logged search of about 400 games takes 7 to 25 minutes on 22 workers,
and reading it and measuring each rule it suggests takes a few hours more.

## Done

The first five pilots were searched against 12 reference decks. The later
rounds use the current 13, excluding a pilot's self-match: Burn, Spellcaster,
Crystal, Morphtronic, and Gishki face 12.
The first five entries show searches before their latest rules; the other
entries show follow-up searches after their rules (104 games each, 96 for
Burn, Spellcaster, Crystal, Morphtronic, and Gishki). Search gaps estimate remaining
headroom; they are not hard ceilings.

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
| Arcana Force Fortune | 2 | 24.0% | 38.5% | 14.4 | 20.73% → 21.82% (the 30 other decks, held out) | [arcana-burn-rules.md](arcana-burn-rules.md) |
| Burn Princess | 2 | 28.1% | 35.4% | 7.3 | 24.83% → 31.52% (the 30 other decks, held out) | [arcana-burn-rules.md](arcana-burn-rules.md) |
| Spellcaster's Command | 2 | 29.2% | 37.5% | 8.3 | 26.14% → 30.67% (the 30 other decks, held out) | [spellcaster-pyramid-rules.md](spellcaster-pyramid-rules.md) |
| Pyramid of Light | 2 | 33.7% | 41.3% | 7.7 | 28.35% → 31.74% (the 30 other decks, held out) | [spellcaster-pyramid-rules.md](spellcaster-pyramid-rules.md) |
| Crystal Beast | 2 | 31.2% | 42.7% | 11.5 | 32.02% → 41.20% (the 30 other decks, held out) | [crystal-morphtronic-rules.md](crystal-morphtronic-rules.md) |
| Morphtronic | 2 | 36.5% | 47.9% | 11.5 | 37.34% → 43.67% (the 30 other decks, held out) | [crystal-morphtronic-rules.md](crystal-morphtronic-rules.md) |
| Gishki | 2 | 39.6% | 50.0% | 10.4 | 39.90% → 49.81% (the 30 other decks, held out) | [gishki-karakuri-rules.md](gishki-karakuri-rules.md) |
| Karakuri | 2 | 46.2% | 72.1% | 26.0 | 44.41% → 51.30% (the 30 other decks, held out) | [gishki-karakuri-rules.md](gishki-karakuri-rules.md) |

A deck is not finished after one search: a second one on the improved
Infernity pilot led to 2.7 points more.  Fortune Lady still has a 19.5-point search gap and has only been searched once.

## To do

The 14 other decks, in the order of the [current tier list](../DECK-TIER-LIST.md)
refreshed on 2026-10-06.
Harpie remains in the ranking pool, but its illegal list must be replaced
and remeasured before policy work.

| # | Deck | Policy | Win rate | Tier | |
|---:|---|---|---:|---|---|
| 1 | Claudi-oh's Countdown | `countdown` | 92.7% | strong |  |
| 2 | Claudi-oh's Verdict | `verdict` | 82.7% | strong |  |
| 4 | Machina Gadgets | `machina` | 71.3% | strong |  |
| 5 | Gravekeeper's Tomb | `gravekeeper` | 68.0% | mid |  |
| 8 | Fusion Heroes | `heroes` | 62.3% | mid |  |
| 9 | Legendary Six Samurai | `six-samurai` | 62.2% | mid |  |
| 11 | Lightsworn Judgment | `lightsworn` | 58.3% | mid |  |
| 12 | Tele-DAD | `tele-dad` | 57.9% | mid |  |
| 13 | 06 Koaki Meiru - Rock Block | `rock-block` | 57.4% | mid |  |
| 14 | Fight, Gladiators! | `gladiator` | 56.1% | mid |  |
| 15 | Dragunity Flight | `dragunity` | 54.4% | mid |  |
| 16 | X-Sabers | `x-saber` | 52.4% | mid |  |
| 17 | Quickdraw Plants | `quickdraw-plant` | 51.6% | mid |  |
| 20 | Harpie Sisters | `harpie` | 48.1% | weak | illegal list; defer policy work |
