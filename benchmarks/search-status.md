# The search, deck by deck

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone helps identify possible improvements,
and its logged games say where to look for rules
([method](search-rules.md#method)).
Search has privileged deck-composition and random-state access, so its gap is
not a guaranteed gain achievable by an ordinary policy. The policies themselves
receive only information their player is permitted to know.

Twenty-two pilots have been through it. The other 9 are to follow, a few at a
time: a logged search of about 400 games takes 7 to 25 minutes on 22 workers,
and reading it and measuring each rule it suggests takes a few hours more.

## Done

The first five pilots were searched against 12 reference decks. The later
rounds use the current 13, excluding a pilot's self-match: Burn, Spellcaster,
Crystal, Morphtronic, Gishki, Dragunity, and Gladiators face 12.
The first five entries show searches before their latest rules; the other
entries show follow-up searches after their rules (104 games each, 96 for
Burn, Spellcaster, Crystal, Morphtronic, Gishki, Dragunity, and Gladiators).
Search gaps estimate remaining headroom; they are not hard ceilings.

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
| Quickdraw Plants | 2 | 60.6% | 79.8% | 19.2 | 51.32% → 56.70% (the 30 other decks, held out) | [quickdraw-xsaber-rules.md](quickdraw-xsaber-rules.md) |
| X-Sabers | 2 | 61.5% | 80.8% | 19.2 | 52.04% → 59.15% (the 30 other decks, held out) | [quickdraw-xsaber-rules.md](quickdraw-xsaber-rules.md) |
| Dragunity Flight | 2 | 63.5% | 81.2% | 17.7 | 54.47% → 63.92% (the 30 other decks, held out) | [dragunity-gladiator-rules.md](dragunity-gladiator-rules.md) |
| Fight, Gladiators! | 2 | 51.0% | 70.8% | 19.8 | 55.83% → 62.46% (the 30 other decks, held out) | [dragunity-gladiator-rules.md](dragunity-gladiator-rules.md) |
| Harpie Sisters | 2 | 57.7% | 70.2% | 12.5 | 57.60% → 61.58% (legal list, the 30 other decks, held out) | [harpie-rules.md](harpie-rules.md) |

A deck is not finished after one search: a second one on the improved
Infernity pilot led to 2.7 points more.  Fortune Lady still has a 19.5-point search gap and has only been searched once.

## To do

The 9 other decks, in the order of the [current tier list](../DECK-TIER-LIST.md)
refreshed on 2026-10-06.

| # | Deck | Policy | Win rate | Tier | |
|---:|---|---|---:|---|---|
| 1 | Claudi-oh's Countdown | `countdown` | 92.3% | strong |  |
| 2 | Claudi-oh's Verdict | `verdict` | 81.8% | strong |  |
| 4 | Machina Gadgets | `machina` | 69.5% | strong |  |
| 5 | Gravekeeper's Tomb | `gravekeeper` | 66.3% | mid |  |
| 9 | Legendary Six Samurai | `six-samurai` | 60.8% | mid |  |
| 12 | Fusion Heroes | `heroes` | 60.3% | mid |  |
| 15 | Lightsworn Judgment | `lightsworn` | 56.4% | mid |  |
| 16 | Tele-DAD | `tele-dad` | 56.0% | mid |  |
| 17 | 06 Koaki Meiru - Rock Block | `rock-block` | 55.8% | mid |  |
