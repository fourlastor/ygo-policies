# Deck tier list

Measured strength of every deck with a policy in this repository: the 13 existing
opponents and the 15 decks added for *Sands of the Duel* (easy / mid / hard).
Every deck played every other deck 256 times (96,768 duels), each piloted
by its own policy, the two decks swapping seats between duels.

- **vs existing** is the win rate against the 13 existing decks, the pool the game
  draws opponents from today, and the yardstick for the tiers.
- **vs all** is the win rate against the other 27 decks.
- **Elo** is a Bradley-Terry rating fitted to all the results (1500 = the average deck).
  It does not depend on which decks are in the pool.

Draws count as half a win. ±: 95% interval.

## Tiers

Tiers follow the win rate against the existing decks. The cut points are placed where
they best reproduce the game's current tiers (`OpponentTiers` in `game/data/balance.json`):
**weak** below 44.7%, **mid** from 44.7% to 64.6%, **strong** above.
With these cuts every existing deck keeps its tier.

- **weak** (14): `destiny-hero`, `ojama`, `arcana`, `watt`, `toon`, `spellcaster`, `pyramid`, `burn`, `crystal`, `fortune-lady`, `morphtronic`, `gishki`, `karakuri`, `harpie`
- **mid** (9): `quickdraw-plant`, `gladiator`, `infernity`, `x-saber`, `dragunity`, `tele-dad`, `lightsworn`, `six-samurai`, `blackwing`
- **strong** (5): `rock-block`, `heroes`, `gravekeeper`, `monarch`, `machina`

The new decks were built as easy, mid and hard opponents (read: weak, mid, strong).
Measured, the easy ones hold; the others mostly land one tier lower:

- **Easy:** Pyramid of Light weak (24.7%), Toon Kingdom weak (21.8%), Watt Grid weak (19.7%), Arcana Force Fortune weak (18.4%), Ojama Brigade weak (17.9%).
- **Mid:** Gravekeeper's Tomb strong (72.3%), Harpie Sisters weak (44.5%), Karakuri Workshop weak (43.1%), Fortune Ladies weak (31.6%), Destiny HEROes weak (16.7%).
- **Hard:** Machina Gadgets strong (75.3%), Legendary Six Samurai mid (63.0%), Tele-DAD mid (59.1%), X-Sabers mid (52.5%), Quickdraw Plants mid (47.7%).

## Ranking

"Planned / game" is the tier a new deck was built for, or the tier an existing deck has in
the game today; "(moves)" marks a deck whose measured tier differs from it.

| # | Deck | Policy | Elo | vs existing | vs all | Tier (measured) | Tier (planned / game) |
|---:|---|---|---:|---:|---:|---|---|
| 1 | Machina Gadgets | `machina` | 1752 | 75.3% ±1.4 | 78.2% ±0.9 | strong | hard (new) |
| 2 | Gravekeeper's Tomb | `gravekeeper` | 1751 | 72.3% ±1.5 | 78.2% ±0.9 | strong (moves) | mid (new) |
| 3 | Emperor, Arise! | `monarch` | 1724 | 75.0% ±1.5 | 75.5% ±0.9 | strong | strong (game) |
| 4 | Fusion Heroes | `heroes` | 1687 | 68.1% ±1.6 | 71.8% ±1.0 | strong | strong (game) |
| 5 | Blackwing Assassin | `blackwing` | 1659 | 64.2% ±1.6 | 68.7% ±1.0 | mid | mid (game) |
| 6 | Legendary Six Samurai | `six-samurai` | 1648 | 63.0% ±1.6 | 67.5% ±1.0 | mid (moves) | hard (new) |
| 7 | 06 Koaki Meiru - Rock Block | `rock-block` | 1648 | 65.0% ±1.6 | 67.5% ±1.0 | strong | strong (game) |
| 8 | Dragunity Flight | `dragunity` | 1628 | 58.8% ±1.6 | 65.2% ±1.0 | mid | mid (game) |
| 9 | Tele-DAD | `tele-dad` | 1618 | 59.1% ±1.6 | 64.1% ±1.0 | mid (moves) | hard (new) |
| 10 | Lightsworn Judgment | `lightsworn` | 1593 | 60.7% ±1.6 | 61.1% ±1.1 | mid | mid (game) |
| 11 | Fight, Gladiators! | `gladiator` | 1575 | 49.0% ±1.5 | 58.9% ±1.0 | mid | mid (game) |
| 12 | X-Sabers | `x-saber` | 1568 | 52.5% ±1.6 | 58.0% ±1.0 | mid (moves) | hard (new) |
| 13 | Infernity Infinity | `infernity` | 1558 | 51.1% ±1.7 | 56.8% ±1.1 | mid | mid (game) |
| 14 | Harpie Sisters | `harpie` | 1539 | 44.5% ±1.5 | 54.5% ±1.0 | weak (moves) | mid (new) |
| 15 | Quickdraw Plants | `quickdraw-plant` | 1520 | 47.7% ±1.6 | 52.2% ±1.1 | mid (moves) | hard (new) |
| 16 | Karakuri Workshop | `karakuri` | 1510 | 43.1% ±1.6 | 50.9% ±1.0 | weak (moves) | mid (new) |
| 17 | Undersea Ceremony | `gishki` | 1496 | 40.4% ±1.5 | 49.1% ±1.0 | weak | weak (game) |
| 18 | 01 Morphtronic - Straight Up | `morphtronic` | 1455 | 36.3% ±1.6 | 44.2% ±1.0 | weak | weak (game) |
| 19 | Fortune Ladies | `fortune-lady` | 1397 | 31.6% ±1.4 | 37.3% ±1.0 | weak (moves) | mid (new) |
| 20 | 12 Crystal Beast - Rainbow | `crystal` | 1391 | 31.2% ±1.6 | 36.6% ±1.1 | weak | weak (game) |
| 21 | 14 - Spellcaster's Command | `spellcaster` | 1351 | 24.0% ±1.3 | 32.1% ±0.9 | weak | weak (game) |
| 22 | Burn Princess | `burn` | 1338 | 26.3% ±1.4 | 30.7% ±1.0 | weak | weak (game) |
| 23 | Pyramid of Light | `pyramid` | 1336 | 24.7% ±1.4 | 30.5% ±0.9 | weak | easy (new) |
| 24 | Toon Kingdom | `toon` | 1297 | 21.8% ±1.3 | 26.4% ±0.9 | weak | easy (new) |
| 25 | Watt Grid | `watt` | 1251 | 19.7% ±1.2 | 22.0% ±0.9 | weak | easy (new) |
| 26 | Destiny HEROes | `destiny-hero` | 1251 | 16.7% ±1.2 | 22.0% ±0.9 | weak (moves) | mid (new) |
| 27 | Arcana Force Fortune | `arcana` | 1231 | 18.4% ±1.3 | 20.2% ±0.9 | weak | easy (new) |
| 28 | Ojama Brigade | `ojama` | 1226 | 17.9% ±1.2 | 19.8% ±0.9 | weak | easy (new) |

## Reading the numbers

- **These are AI-against-AI results.** A human exploits different weaknesses. The largest
  gap is Gravekeeper's Tomb (72.3% against the existing decks): Necrovalley
  shuts down most of their Graveyard engines, and no policy ever removes a face-up Field
  Spell (the shared Mystical Space Typhoon rule only targets Set cards). Against a player
  who destroys Necrovalley, expect it to play closer to its planned mid tier.
- **Destiny HEROes is weak because of its list** (16.7%): five Level 6 and two
  Level 8 monsters leave many opening hands with nothing to summon unless Destiny Draw
  shows up. Trading some Level 6s for Level 4s would move it toward its planned mid tier.
- **Machina Gadgets' strength is the deck's.** The shared agent alone already wins about
  as much (75.1%) as its policy (75.3%).
- **Life Points.** Every duel here starts at 8000. The game's duels, elites and bosses start
  at 4000, 6000 and 8000. Lower Life Points should favour fast damage (Watt Grid, Toon
  Kingdom, Six Samurai) over slow engines (Destiny HEROes, Fortune Ladies); not measured.
- **Arcana Force Fortune** plays without knowing its coin results (the projection does not
  track them): Second Coin Toss redoes tosses blindly, Reversal of Fate and Arcana Call
  stay unused.
- **The existing 13 policies play as before.** Twelve reproduce every game exactly. Dragunity
  now plays its Shrink through the new shared staple: it changes the course of about 4% of
  its games and the result of 30 out of 3,072, with no change in strength (59.15% to 59.28%).

## What the new policies add

Win rate of each new deck against the 13 existing decks, piloted by the shared
agent alone (summons, attacks and the shared staple cards, no deck policy) and by
its own policy.

| Deck | Planned | Generic pilot | Policy | Change |
|---|---|---:|---:|---:|
| Pyramid of Light | easy | 11.9% | 24.7% | +12.8 |
| Toon Kingdom | easy | 12.0% | 21.8% | +9.8 |
| Watt Grid | easy | 14.3% | 19.7% | +5.4 |
| Arcana Force Fortune | easy | 15.1% | 18.4% | +3.3 |
| Ojama Brigade | easy | 12.4% | 17.9% | +5.5 |
| Gravekeeper's Tomb | mid | 23.1% | 72.3% | +49.2 |
| Harpie Sisters | mid | 30.5% | 44.5% | +13.9 |
| Karakuri Workshop | mid | 33.4% | 43.1% | +9.7 |
| Fortune Ladies | mid | 16.9% | 31.6% | +14.7 |
| Destiny HEROes | mid | 7.3% | 16.7% | +9.4 |
| Machina Gadgets | hard | 75.1% | 75.3% | +0.2 |
| Legendary Six Samurai | hard | 37.0% | 63.0% | +26.0 |
| Tele-DAD | hard | 37.1% | 59.1% | +22.0 |
| X-Sabers | hard | 33.9% | 52.5% | +18.6 |
| Quickdraw Plants | hard | 32.0% | 47.7% | +15.7 |

## Win-rate matrix

Row deck's win rate against the column deck, in percent.

| | mach | gk | mon | hero | bw | 6sam | rock | drag | dad | ls | glad | xsab | inf | harp | plant | kara | gish | morph | fl | cry | spell | burn | pyr | toon | watt | dhero | arc | ojama |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **mach** | – | 42 | 64 | 63 | 70 | 70 | 64 | 71 | 62 | 65 | 82 | 72 | 69 | 77 | 77 | 82 | 78 | 88 | 92 | 82 | 96 | 86 | 98 | 95 | 85 | 91 | 94 | 98 |
| **gk** | 58 | – | 52 | 61 | 62 | 55 | 69 | 63 | 72 | 71 | 56 | 81 | 81 | 65 | 84 | 84 | 82 | 82 | 93 | 74 | 92 | 95 | 94 | 96 | 97 | 98 | 97 | 97 |
| **mon** | 36 | 48 | – | 49 | 73 | 65 | 61 | 65 | 58 | 75 | 82 | 60 | 68 | 93 | 59 | 74 | 86 | 80 | 85 | 75 | 97 | 88 | 90 | 91 | 94 | 98 | 93 | 96 |
| **hero** | 37 | 39 | 51 | – | 53 | 60 | 56 | 55 | 62 | 58 | 77 | 65 | 57 | 84 | 70 | 61 | 88 | 74 | 87 | 80 | 91 | 77 | 89 | 91 | 96 | 92 | 91 | 96 |
| **bw** | 30 | 38 | 27 | 47 | – | 37 | 65 | 62 | 51 | 40 | 71 | 61 | 58 | 74 | 71 | 73 | 72 | 73 | 87 | 66 | 97 | 92 | 91 | 91 | 96 | 95 | 94 | 95 |
| **6sam** | 30 | 45 | 35 | 40 | 63 | – | 49 | 56 | 52 | 62 | 66 | 67 | 73 | 62 | 61 | 77 | 64 | 69 | 80 | 70 | 83 | 88 | 91 | 82 | 81 | 91 | 91 | 92 |
| **rock** | 36 | 31 | 39 | 44 | 35 | 51 | – | 80 | 51 | 60 | 69 | 59 | 64 | 54 | 67 | 77 | 79 | 79 | 93 | 79 | 87 | 64 | 89 | 84 | 79 | 89 | 90 | 93 |
| **drag** | 29 | 37 | 35 | 45 | 38 | 44 | 20 | – | 48 | 45 | 58 | 60 | 62 | 69 | 61 | 62 | 76 | 84 | 82 | 72 | 86 | 86 | 90 | 93 | 96 | 93 | 93 | 98 |
| **dad** | 38 | 28 | 42 | 38 | 49 | 48 | 49 | 52 | – | 46 | 64 | 55 | 52 | 72 | 60 | 57 | 66 | 80 | 77 | 67 | 89 | 73 | 93 | 85 | 80 | 86 | 88 | 93 |
| **ls** | 35 | 29 | 25 | 42 | 60 | 38 | 40 | 55 | 54 | – | 70 | 63 | 64 | 66 | 61 | 69 | 76 | 77 | 72 | 73 | 76 | 71 | 68 | 69 | 79 | 75 | 68 | 75 |
| **glad** | 18 | 44 | 18 | 23 | 29 | 34 | 31 | 42 | 36 | 30 | – | 50 | 45 | 60 | 59 | 65 | 54 | 65 | 84 | 71 | 86 | 94 | 86 | 93 | 96 | 97 | 89 | 89 |
| **xsab** | 28 | 19 | 40 | 35 | 39 | 33 | 41 | 40 | 45 | 37 | 50 | – | 42 | 48 | 53 | 62 | 57 | 68 | 76 | 65 | 82 | 85 | 82 | 78 | 91 | 88 | 91 | 93 |
| **inf** | 31 | 19 | 32 | 43 | 42 | 27 | 36 | 38 | 48 | 36 | 55 | 58 | – | 55 | 61 | 62 | 59 | 64 | 71 | 60 | 85 | 63 | 77 | 81 | 75 | 87 | 88 | 81 |
| **harp** | 23 | 35 | 7 | 16 | 26 | 38 | 46 | 31 | 28 | 34 | 40 | 52 | 45 | – | 55 | 65 | 42 | 63 | 69 | 64 | 71 | 92 | 78 | 93 | 97 | 93 | 83 | 86 |
| **plant** | 23 | 16 | 41 | 30 | 29 | 39 | 33 | 39 | 40 | 39 | 41 | 47 | 39 | 45 | – | 38 | 58 | 64 | 64 | 66 | 77 | 63 | 78 | 75 | 68 | 73 | 91 | 93 |
| **kara** | 18 | 16 | 26 | 39 | 27 | 23 | 23 | 38 | 43 | 31 | 35 | 38 | 38 | 35 | 62 | – | 56 | 45 | 77 | 55 | 71 | 77 | 76 | 68 | 94 | 85 | 87 | 92 |
| **gish** | 22 | 18 | 14 | 12 | 28 | 36 | 21 | 24 | 34 | 24 | 46 | 43 | 41 | 58 | 42 | 44 | – | 61 | 71 | 61 | 75 | 77 | 75 | 79 | 87 | 79 | 79 | 77 |
| **morph** | 12 | 18 | 20 | 26 | 27 | 31 | 21 | 16 | 20 | 23 | 35 | 32 | 36 | 37 | 36 | 55 | 39 | – | 55 | 68 | 53 | 71 | 67 | 81 | 78 | 77 | 85 | 74 |
| **fl** | 8 | 7 | 15 | 13 | 13 | 20 | 7 | 18 | 23 | 28 | 16 | 24 | 29 | 31 | 36 | 23 | 29 | 45 | – | 54 | 66 | 78 | 54 | 74 | 72 | 76 | 78 | 69 |
| **cry** | 18 | 26 | 25 | 20 | 34 | 30 | 21 | 28 | 33 | 27 | 29 | 35 | 40 | 36 | 34 | 45 | 39 | 32 | 46 | – | 55 | 26 | 52 | 38 | 41 | 59 | 57 | 64 |
| **spell** | 4 | 8 | 3 | 9 | 3 | 17 | 13 | 14 | 11 | 24 | 14 | 18 | 15 | 29 | 23 | 29 | 25 | 47 | 34 | 45 | – | 75 | 47 | 74 | 80 | 70 | 65 | 70 |
| **burn** | 14 | 5 | 12 | 23 | 8 | 12 | 36 | 14 | 27 | 29 | 6 | 15 | 37 | 8 | 37 | 23 | 23 | 29 | 22 | 74 | 25 | – | 57 | 51 | 46 | 70 | 70 | 58 |
| **pyr** | 2 | 6 | 10 | 11 | 9 | 9 | 11 | 10 | 7 | 32 | 14 | 18 | 23 | 22 | 22 | 24 | 25 | 33 | 46 | 48 | 53 | 43 | – | 71 | 71 | 58 | 71 | 74 |
| **toon** | 5 | 4 | 9 | 9 | 9 | 18 | 16 | 7 | 15 | 31 | 7 | 22 | 19 | 7 | 25 | 32 | 21 | 19 | 26 | 62 | 26 | 49 | 29 | – | 77 | 52 | 63 | 54 |
| **watt** | 15 | 3 | 6 | 4 | 4 | 19 | 21 | 4 | 20 | 21 | 4 | 9 | 25 | 3 | 32 | 6 | 13 | 22 | 28 | 59 | 20 | 54 | 29 | 23 | – | 59 | 51 | 41 |
| **dhero** | 9 | 2 | 2 | 8 | 5 | 9 | 11 | 7 | 14 | 25 | 3 | 12 | 13 | 7 | 27 | 15 | 21 | 23 | 24 | 41 | 30 | 30 | 42 | 48 | 41 | – | 60 | 65 |
| **arc** | 6 | 3 | 7 | 9 | 6 | 9 | 10 | 7 | 12 | 32 | 11 | 9 | 12 | 17 | 9 | 13 | 21 | 15 | 22 | 43 | 35 | 30 | 29 | 37 | 49 | 40 | – | 52 |
| **ojama** | 2 | 3 | 4 | 4 | 5 | 8 | 7 | 2 | 7 | 25 | 11 | 7 | 19 | 14 | 7 | 8 | 23 | 26 | 31 | 36 | 30 | 42 | 26 | 46 | 59 | 35 | 48 | – |

## Method

- Engine: OCGCore as built by the host repository (`ygo`, `YGO.TrainingWorker`) with its
  `cards.cdb` and card scripts. Each seat is one of this repository's policies, fed the raw
  message stream (it hides what its player may not see itself).
- Every pair of decks played 256 duels at 8000 Life Points, the two decks swapping seats
  between games. Seeds are fixed, so the run is reproducible.
- A duel past 4096 decisions would count as a draw; none got there, and 0 failed.
- The generic-pilot column comes from a separate run of 64 duels against each existing deck.

