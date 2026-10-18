# The search, deck by deck

`policy-bench search` plays a pilot with a one-step search on top
([how](../crates/ygo-policies-bench/README.md#a-search-on-top-of-a-pilot)).
What the search wins over the pilot alone helps identify possible improvements,
and its logged games say where to look for rules
([method](search-rules.md#method)).
Search has privileged deck-composition and random-state access, so its gap is
not a guaranteed gain achievable by an ordinary policy. The policies themselves
receive only information their player is permitted to know.

Sixty-nine of the 71 pilots have now been searched. The 39 additions completed
an initial diagnostic and measured optimization pass at a maximum of eight
workers. Older reports retain their originally recorded worker counts.

## Done

### Original roster

The first five pilots were searched against 12 reference decks. The later
rounds use the current 13, excluding a pilot's self-match: Burn, Spellcaster,
Crystal, Morphtronic, Gishki, Dragunity, Gladiators, Rock Block, Lightsworn, and Heroes face 12.
The first five entries show searches before their latest rules; the other
entries show follow-up searches after their rules (104 games each, 96 for
Burn, Spellcaster, Crystal, Morphtronic, Gishki, Dragunity, Gladiators, Rock Block, Lightsworn, and Heroes).
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
| Rock Block | 2 | 58.3% | 72.9% | 14.6 | 55.48% → 61.54% (the 30 other decks, held out) | [rock-tele-rules.md](rock-tele-rules.md) |
| Tele-DAD | 2 | 58.7% | 76.0% | 17.3 | 55.42% → 57.77% (the 30 other decks, held out) | [rock-tele-rules.md](rock-tele-rules.md) |
| Lightsworn Judgment | 2 | 68.8% | 84.4% | 15.6 | 56.19% → 63.95% (the 30 other decks, held out) | [lightsworn-heroes-rules.md](lightsworn-heroes-rules.md) |
| Fusion Heroes | 2 | 71.9% | 81.2% | 9.4 | 59.40% → 68.15% (the 30 other decks, held out) | [lightsworn-heroes-rules.md](lightsworn-heroes-rules.md) |
| Legendary Six Samurai | 2 | 73.1% | 90.4% | 17.3 | 59.51% → 70.83% (the 30 other decks, held out) | [samurai-gravekeeper-machina-rules.md](samurai-gravekeeper-machina-rules.md) |
| Gravekeeper's Tomb | 2 | 60.6% | 77.9% | 17.3 | 65.41% → 67.25% (the 30 other decks, held out) | [samurai-gravekeeper-machina-rules.md](samurai-gravekeeper-machina-rules.md) |
| Machina Gadgets | 2 | 81.7% | 92.3% | 10.6 | 70.36% → 78.40% (the 30 other decks, held out) | [samurai-gravekeeper-machina-rules.md](samurai-gravekeeper-machina-rules.md) |
| Astra's Exodia | 2 | 69.2% | 77.9% | 8.7 | 80.24% → 78.48% (faster Heart replacement, held out) | [exodia.md](exodia.md) |

A deck is not finished after one search: a second one on the improved
Infernity pilot led to 2.7 points more.  Fortune Lady still has a 19.5-point search gap and has only been searched once.

### Expanded roster: first optimization pass

Each initial policy was searched for 52 games against the 13 reference decks,
seed 920000, using 4/12/24 worlds. These are **before-tuning diagnostics**, not
follow-up searches on the final policies or hard ceilings. All 2,028 games
finished without errors, decision caps or failed rollouts.

The rules column is a separate fresh-seed paired comparison against all 32
original policies: 2,048 pairs per addition, seed 950000. The opponent code and
decklists are fixed. "Improved" means the individual paired 95% interval is
above zero; intervals are exploratory and are not adjusted for 39 comparisons.
"No clear gain" retains useful behavior changes without claiming a measured
strength gain. See [the method and full intervals](new-decks-optimization.md).

| Deck | Policy | Initial search: alone | With search | Gap | Held-out rules: before → after | Result |
|---|---|---:|---:|---:|---|---|
| Fabled Encore | `fabled` | 38.5% | 48.1% | 9.6 | 39.70% → 39.79% | No clear gain |
| Heaven's Rebuttal | `counter-fairy` | 48.1% | 59.6% | 11.5 | 49.22% → 52.20% | Improved |
| D.D. Border Patrol | `macro-dd` | 65.4% | 82.7% | 17.3 | 68.16% → 68.16% | Unchanged |
| Gusto's Reprisal | `gusto` | 44.2% | 76.9% | 32.7 | 50.49% → 51.51% | No clear gain |
| Heaven's Dispatch | `agents` | 48.1% | 69.2% | 21.2 | 58.35% → 64.94% | Improved |
| Scrap Renaissance | `scrap` | 61.5% | 82.7% | 21.2 | 66.50% → 67.63% | Improved |
| Graveyard Shift | `zombie` | 50.0% | 71.2% | 21.2 | 45.41% → 47.51% | Improved |
| Herald's Veto | `herald` | 40.4% | 48.1% | 7.7 | 28.37% → 32.76% | Improved |
| Tidal Assembly | `fish` | 46.2% | 57.7% | 11.5 | 51.27% → 52.15% | Improved |
| Power Surge | `cyber` | 17.3% | 23.1% | 5.8 | 23.14% → 23.14% | Unchanged |
| Second Bloom | `gemini` | 67.3% | 84.6% | 17.3 | 56.98% → 57.18% | No clear gain |
| Mind Over Matter | `psychic` | 51.9% | 71.2% | 19.2 | 45.31% → 48.73% | Improved |
| Last Page | `deckout` | 36.5% | 44.2% | 7.7 | 37.30% → 49.19% | Improved |
| Chain Reaction | `chain-burn` | 26.9% | 32.7% | 5.8 | 27.29% → 27.88% | Improved |
| A Bitter Cure | `nurse` | 30.8% | 42.3% | 11.5 | 39.11% → 39.11% | Unchanged |
| A Thousand Blades | `benkei` | 32.7% | 42.3% | 9.6 | 31.30% → 35.11% | Improved |
| Visitors from Beyond | `alien` | 59.6% | 76.9% | 17.3 | 50.83% → 58.35% | Improved |
| Passing Spirits | `spirit` | 23.1% | 42.3% | 19.2 | 28.86% → 28.86% | No clear gain |
| The Quiet Grove | `naturia` | 46.2% | 73.1% | 26.9 | 53.54% → 55.32% | Improved |
| Garden of Thorns | `garden` | 42.3% | 59.6% | 17.3 | 41.89% → 45.61% | Improved |
| The Final Sentence | `destiny-board` | 40.4% | 53.8% | 13.5 | 49.22% → 50.98% | Improved |
| Crown of Venom | `venom` | 15.4% | 17.3% | 1.9 | 16.06% → 16.06% | Unchanged |
| Full Charge | `batteryman` | 51.9% | 59.6% | 7.7 | 42.04% → 48.73% | Improved |
| Prismatic Forge | `gem-knight` | 46.2% | 48.1% | 1.9 | 44.34% → 45.02% | No clear gain |
| Ashes to Inferno | `flamvell` | 28.8% | 51.9% | 23.1 | 43.02% → 44.68% | Improved |
| Road to Ragnarok | `nordic` | 57.7% | 80.8% | 23.1 | 53.08% → 59.47% | Improved |
| Eye of the Storm | `cloudian` | 42.3% | 55.8% | 13.5 | 48.78% → 49.80% | No clear gain |
| Volcanic Aftershock | `volcanic` | 44.2% | 55.8% | 11.5 | 42.19% → 49.41% | Improved |
| Gates Unopened | `dark-world` | 46.2% | 53.8% | 7.7 | 55.76% → 57.18% | Improved |
| Visitors Beneath | `worm` | 38.5% | 55.8% | 17.3 | 37.01% → 41.46% | Improved |
| Between Light and Dark | `chaos` | 65.4% | 82.7% | 17.3 | 64.60% → 67.19% | Improved |
| Armageddon Hour | `demise` | 44.2% | 57.7% | 13.5 | 50.49% → 51.27% | No clear gain |
| Queens of the Wild | `amazoness` | 42.3% | 69.2% | 26.9 | 46.14% → 51.86% | Improved |
| Footprints in Fire | `jurrac` | 36.5% | 59.6% | 23.1 | 51.86% → 51.61% | No clear gain |
| Clockwork Current | `genex` | 53.8% | 78.8% | 25.0 | 48.78% → 49.22% | No clear gain |
| Winter Parliament | `ice-barrier` | 30.8% | 50.0% | 19.2 | 36.65% → 42.63% | Improved |
| The Still Gaze | `reptilianne` | 46.2% | 61.5% | 15.4 | 44.53% → 44.58% | No clear gain |
| Rust Never Sleeps | `iron-chain` | 28.8% | 57.7% | 28.8 | 35.45% → 41.94% | Improved |
| Eclipse Without End | `malefic` | 48.1% | 67.3% | 19.2 | 50.63% → 56.98% | Improved |

## To do

The 2 other decks, in the order of the [current tier list](../DECK-TIER-LIST.md)
refreshed on 2026-10-08 across all 71 decks.

| # | Deck | Policy | Win rate | Tier | |
|---:|---|---|---:|---|---|
| 1 | Claudi-oh's Countdown | `countdown` | 88.1% | strong |  |
| 2 | Claudi-oh's Verdict | `verdict` | 79.8% | strong |  |

The expanded policies have not yet had follow-up searches after these rules.
The initial gaps above remain a guide for the next pass, especially where the
paired comparison found no clear gain. Venom's small diagnostic gap alongside
its low win rate suggests revisiting its list as well as its pilot.
