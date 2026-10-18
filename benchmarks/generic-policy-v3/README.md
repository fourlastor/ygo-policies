# Generic policy: the staged search (v3)

Work on `codex/fast-generic-policy` after astra's v2 experiments
([v2 README](../generic-policy-v2/README.md)), started 2026-10-17. First the search
on top of the generic pilot changes, then (2026-10-18) the pilot itself.

**Result.** On the whole roster (70 decks, 32 games each) the staged search wins
62.7%, against 51.5% for astra's fast MCTS and 36.9% for the plain pilot; 51 of
70 decks are at 55% or more (astra's MCTS: 27). The per-deck target is not met:
19 decks are under 55%, but only Chain Burn, Exodia and Deck-out clearly so (the
upper end of their 95% interval is under 55%); the other 16 are at 14 to 17 wins
of 32. On the 16 decks where the generic was weakest it goes from 36.3% (plain)
and 43.8% (astra's MCTS) to 59.4%. No knowledge of the opponent's policy is
used: both players of every simulated world are the generic pilot.

**With a better pilot** ([below](#a-better-pilot)) the same search wins 65.3% of
the whole roster, with 57 of 70 decks at 55% or more; only Exodia is clearly
under (11 of 32).

## Why v2 stalled: the search decided on noise

At each decision astra's information-set MCTS gives its alternatives about four
noisy win/loss results each (32 iterations over up to 8 actions), then checks the
proposal against the pilot's answer on four fresh worlds and takes it on any
lead. Against plain play on the 16 decks below it gained 95 games and lost 57:
a third of what it changed, it changed for the worse. Scoring its simulations by
game results instead of the 13-feature evaluator did not help (41.8% against
43.8%, at the compute of the staged search), and the staged search with twice
the worlds gained only +2.9 points (not significant). The evaluator and the
budget were not what held it back.

`--method staged` uses the choice `policy-bench search` already made for the
specialists, on astra's information-safe worlds: every alternative is played to
the end of the duel, both players Generic (the generic opponent model), in the
same sampled worlds as the pilot's answer, which is tried first. After each of
the stages (8, 24, 64 worlds) only the alternatives ahead of the pilot's go on,
the best two after the first; the pilot's answer is left only for one ahead by
a paired z of 1.645 after the last stage. Against plain play it gained 87 games
and lost 18.

## Two decisions the search did not see

astra's guards left every decision inside an open chain, and every decision
taken while the other player had an unknown face-down monster, to the pilot. On
the stall decks that is most of the game: in Exodia's mirror the search ran on 3
decisions a game and skipped 130 for face-down monsters.

- `--search-chains true` searches inside open chains. The engine's hidden-card
  swap refuses any card a link of the chain is handled by or targets; selections
  that name hidden cards stay with the pilot, as before.
- `--resample-set-monsters true` deals the other player's unknown Set monsters
  again: each becomes any monster of that player's unknown cards that it could
  have Set there (a Main Deck monster of Level 4 or lower, not Ritual), drawn
  without looking at which it really is. The engine's swap
  (`vendor/ocgcore/duel_swap.cpp`) now exchanges a face-down monster that was
  Normal Set without a Tribute or Special Summoned face-down, with nothing
  attached to it, with a monster of the hand or the Deck; the zone keeps its
  position, turn, summon and attack state. A root whose Set monster cannot be
  swapped (a Tribute Set, or something attached) stays with the pilot and is
  counted in `undealt_roots`.

A deliberately cheating oracle that searches facing face-down monsters with
their real identity in every world measures the ceiling of the second change:
+5.5 points. The fair version reaches +4.1 of it.

## The 16 hardest decks

The 16 decks where astra's v17 screen had the generic weakest, 32 games each
(16 per seat), seed 2600000, the same deals in every arm. Frozen specialists
from `7ddad06` (they reproduce astra's frozen baseline game for game), the
generic pilot at `0a3c26c`.

| Arm | Wins | Rate |
| --- | ---: | ---: |
| Plain generic | 186/512 | 36.3% |
| astra's MCTS, 32 iterations (v17 fast setting) | 224/512 | 43.8% |
| The same MCTS scored by game results | 214/512 | 41.8% |
| Staged search | 255/512 | 49.8% |
| Staged search, twice the worlds and 12 alternatives | 270/512 | 52.7% |
| Staged search inside open chains | 278/512 | 54.3% |
| Staged search dealing Set monsters again (first engine version) | 276/512 | 53.9% |
| Both (first engine version) | 301/512 | 58.8% |
| **Final code: both, the swap also taking face-down Special Summons** | 304/512 | **59.4%** |
| Benchmark only: staged search, the specialist as opponent model | 275/512 | 53.7% |
| Ceiling: staged search, cheating face-down oracle | 283/512 | 55.3% |

Paired differences over the same 512 games (normal 95% intervals):

| Comparison | Difference | Gained / lost |
| --- | ---: | ---: |
| MCTS against plain | +7.4 [+2.7, +12.1] | 95 / 57 |
| Staged against plain | +13.5 [+9.7, +17.2] | 87 / 18 |
| Staged against MCTS | +6.1 [+1.6, +10.5] | 83 / 52 |
| Staged against MCTS scored by game results | +8.0 [+4.0, +12.1] | 78 / 37 |
| Twice the worlds against staged | +2.9 [-0.6, +6.5] | 51 / 36 |
| Open chains against staged | +4.5 [+2.0, +7.0] | 33 / 10 |
| Set monsters dealt again against staged | +4.1 [+2.0, +6.2] | 26 / 5 |
| Set monsters dealt again against the oracle | -1.4 [-3.9, +1.2] | 19 / 26 |
| Both against plain | +22.5 [+18.1, +26.8] | 135 / 20 |
| Both against MCTS | +15.0 [+10.3, +19.8] | 121 / 44 |
| Specialist opponent model against generic | +3.9 [+0.1, +7.7] | 60 / 40 |

The final code against plain: +23.0 [+18.7, +27.4] points (137 gained, 19 lost); against astra's MCTS: +15.6 [+10.8, +20.4] (124 gained, 44 lost).

Per deck (wins of 32):

| Deck | Plain | MCTS | Staged | + chains | + Set monsters | Both | Final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| blackwing | 12 | 15 | 17 | 18 | 18 | 19 | 19 |
| burn | 8 | 8 | 11 | 12 | 14 | 14 | 14 |
| chain-burn | 11 | 6 | 11 | 15 | 11 | 15 | 15 |
| crystal | 12 | 21 | 23 | 23 | 23 | 25 | 25 |
| deckout | 5 | 5 | 5 | 8 | 6 | 8 | 11 |
| demise | 23 | 23 | 27 | 28 | 28 | 28 | 28 |
| draconic-might | 8 | 15 | 15 | 15 | 17 | 18 | 18 |
| exodia | 14 | 14 | 16 | 16 | 18 | 19 | 19 |
| gishki | 16 | 17 | 18 | 23 | 17 | 23 | 23 |
| heroes | 11 | 14 | 14 | 17 | 15 | 19 | 19 |
| lightsworn | 15 | 14 | 13 | 14 | 18 | 19 | 19 |
| monarch | 9 | 16 | 17 | 18 | 18 | 18 | 18 |
| psychic | 11 | 17 | 21 | 22 | 23 | 24 | 24 |
| pyramid | 5 | 11 | 7 | 9 | 10 | 11 | 11 |
| rock-block | 11 | 10 | 20 | 21 | 20 | 22 | 22 |
| volcanic | 15 | 18 | 20 | 19 | 20 | 19 | 19 |
| **total** | **186** | **224** | **255** | **278** | **276** | **301** | **304** |

The specialist opponent model knows the other player's policy. The user's
decision (2026-10-17): useful as a benchmark, but the final policy does not
assume it. Every other arm uses the generic opponent model.

The swap in its first version took only Normal Sets. Deck-out's Morphing Jar
#2 Special Summons face-down: that version refused 786 of its roots. Taking
face-down Special Summons too, Deck-out went from 6 to 10 of 32 on the same
deals (the oracle: 12) and the other four face-down-heavy decks did not change
(`hard16/fd2-resample`, five decks, chains off).

## The whole roster

Every deck of the catalog, 32 games each (16 per seat), seed 2700000; Countdown is reported but exempt.

| Arm | Wins (70 decks) | Rate | Decks at 55% or more | Wilson lower bound at 55% | Wilson upper bound under 55% |
| --- | ---: | ---: | ---: | ---: | ---: |
| Plain generic | 826/2240 | 36.9% | 5 | 0 | 43 |
| astra's MCTS, 32 iterations | 1154/2240 | 51.5% | 27 | 5 | 11 |
| **Staged search, final settings** | 1405/2240 | 62.7% | 51 | 13 | 3 |

Paired over the 2,240 required games: the staged search +25.8 [+23.6, +28.1] points over plain (686 gained, 107 lost), +11.2 [+8.9, +13.5] over astra's MCTS (484 gained, 233 lost).

Per deck (wins of 32), weakest first under the staged search:

| Deck | Plain | MCTS | Staged |
| --- | ---: | ---: | ---: |
| chain-burn | 8 | 7 | 11 |
| exodia | 11 | 10 | 11 |
| deckout | 6 | 3 | 12 |
| batteryman | 11 | 17 | 14 |
| harpie | 10 | 17 | 14 |
| burn | 7 | 6 | 15 |
| gravekeeper | 10 | 8 | 15 |
| machina | 7 | 14 | 15 |
| monarch | 14 | 13 | 15 |
| morphtronic | 10 | 17 | 15 |
| volcanic | 10 | 15 | 15 |
| pyramid | 7 | 11 | 16 |
| destiny-board | 9 | 9 | 17 |
| fish | 9 | 18 | 17 |
| gladiator | 11 | 18 | 17 |
| infernity | 8 | 17 | 17 |
| reptilianne | 16 | 14 | 17 |
| six-samurai | 10 | 13 | 17 |
| toon | 11 | 12 | 17 |
| benkei | 4 | 15 | 18 |
| countdown (exempt) | 16 | 3 | 18 |
| destiny-hero | 6 | 14 | 18 |
| gusto | 9 | 13 | 18 |
| spellcaster | 6 | 14 | 18 |
| verdict | 14 | 14 | 18 |
| worm | 12 | 14 | 18 |
| arcana | 8 | 16 | 19 |
| dragunity | 10 | 16 | 19 |
| gem-knight | 17 | 17 | 19 |
| genex | 9 | 15 | 19 |
| heroes | 10 | 13 | 19 |
| nordic | 8 | 12 | 19 |
| nurse | 14 | 13 | 19 |
| venom | 12 | 17 | 19 |
| agents | 16 | 17 | 20 |
| gishki | 13 | 18 | 20 |
| iron-chain | 9 | 20 | 20 |
| scrap | 13 | 12 | 20 |
| blackwing | 13 | 17 | 21 |
| draconic-might | 12 | 19 | 21 |
| flamvell | 17 | 18 | 21 |
| lightsworn | 12 | 13 | 21 |
| alien | 12 | 19 | 22 |
| counter-fairy | 15 | 17 | 22 |
| dark-world | 10 | 16 | 22 |
| demise | 12 | 17 | 22 |
| fortune-lady | 7 | 22 | 22 |
| gemini | 11 | 21 | 22 |
| ice-barrier | 15 | 20 | 22 |
| jurrac | 21 | 24 | 22 |
| macro-dd | 17 | 21 | 22 |
| malefic | 11 | 20 | 22 |
| ojama | 8 | 14 | 22 |
| rock-block | 14 | 17 | 22 |
| tele-dad | 10 | 14 | 22 |
| chaos | 10 | 18 | 23 |
| cloudian | 16 | 28 | 23 |
| spirit | 7 | 20 | 23 |
| amazoness | 17 | 21 | 25 |
| karakuri | 8 | 20 | 25 |
| naturia | 13 | 17 | 25 |
| quickdraw-plant | 18 | 21 | 25 |
| watt | 15 | 11 | 25 |
| x-saber | 14 | 21 | 25 |
| crystal | 16 | 23 | 26 |
| cyber | 11 | 21 | 26 |
| garden | 14 | 19 | 26 |
| psychic | 13 | 18 | 26 |
| herald | 23 | 28 | 27 |
| fabled | 18 | 26 | 28 |
| zombie | 21 | 24 | 30 |

## Headroom: the same search with each deck's own specialist

Is 55% out of reach in the mirrors where the generic is weakest, or is the
generic pilot what holds them back? `run.sh headroom` runs the final search
with the deck's own specialist in place of the generic pilot (`--pilot
specialist`): it plays the searching seat and that seat's side of every
simulated game. The specialist is also the opponent model. This is a benchmark
only, because that model is then exactly the opponent. `run.sh
specialist-plain` is the control: the same specialist alone. Both cover the 19
decks under 55% in the roster run, on its deals (seed 2700000, 32 games each).
The specialists come from this branch's build, which can take the search's
overrides. In plain play they replay the frozen `7ddad06` specialists in all
1,216 mirror games checked (64 per deck).

Wins of 32 on the same deals, by the arm's result:

| Deck | Generic plain | Generic search | Specialist alone | Specialist search | 95% interval | Search vs alone |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| destiny-board | 9 | 17 | 12 | 17 | 36–69% | +6 / −1 |
| burn | 7 | 15 | 15 | 18 | 39–72% | +4 / −1 |
| chain-burn | 8 | 11 | 15 | 18 | 39–72% | +5 / −2 |
| harpie | 10 | 14 | 13 | 19 | 42–74% | +8 / −2 |
| exodia | 11 | 11 | 16 | 20 | 45–77% | +4 / −0 |
| batteryman | 11 | 14 | 19 | 21 | 48–80% | +4 / −2 |
| machina | 7 | 15 | 13 | 22 | 51–82% | +9 / −0 |
| six-samurai | 10 | 17 | 16 | 22 | 51–82% | +6 / −0 |
| volcanic | 10 | 15 | 14 | 22 | 51–82% | +9 / −1 |
| fish | 9 | 17 | 12 | 23 | 55–84% | +13 / −2 |
| gravekeeper | 10 | 15 | 17 | 23 | 55–84% | +9 / −3 |
| infernity | 8 | 17 | 15 | 23 | 55–84% | +11 / −3 |
| morphtronic | 10 | 15 | 18 | 23 | 55–84% | +5 / −0 |
| pyramid | 7 | 16 | 18 | 24 | 58–87% | +6 / −0 |
| deckout | 6 | 12 | 16 | 25 | 61–89% | +11 / −2 |
| monarch | 14 | 15 | 21 | 25 | 61–89% | +5 / −1 |
| reptilianne | 16 | 17 | 20 | 25 | 61–89% | +6 / −1 |
| toon | 11 | 17 | 17 | 25 | 61–89% | +9 / −1 |
| gladiator | 11 | 17 | 14 | 29 | 76–97% | +16 / −1 |
| **total** | 30.4% | 47.2% | 49.5% | **69.7%** | | +146 / −23 |

- **These mirrors have room above 55%.** With a pilot that knows the deck,
  the same search wins 69.7%: 20.2 points over the specialist alone on the
  same deals. It gains on every deck, and 18 of the 19 decks reach 18 of 32 or
  better (Destiny Board: 17). The 95% interval is above 50% on 13 decks.
- **The gap is the pilot.** On these decks the generic with search plays like
  the specialist alone: −2.3 points on the same deals (87 games gained, 101
  lost), and 22.5 points under the specialist with search.
- **It is an upper bound.** The opponent model here is exactly the opponent.
  In a mirror, though, a generic pilot that played the deck as well would also
  model the opponent that well: the generic opponent model is that same pilot.
- **The smallest gains** over the specialist alone are on Batteryman (+2 net),
  Chain Burn and Burn (+3), and Exodia and Monarch (+4).
- **The specialists pass too, at the same windows.** Their searches correct
  the same passes as the generic's. Of 1,386 overrides, the most frequent are
  Waboku 26, Solemn Warning 24, Compulsory Evacuation Device 24, Threatening
  Roar 24 and Book of Moon 17. These staples' shared timing (`staples.rs`) is
  where both pilots fall short.
- **Deal luck.** The specialist alone, a 50% policy in its own mirror, won
  between 12 and 21 of these 32 deals depending on the deck. Results on the
  same deal correlate at only 0.38 to 0.49 across arms, so a same-deal control
  removes little of that noise.

## A better pilot

The headroom run put the gap in the generic pilot, so the pilot came next.
Two benchmark-only tools showed where it falls short of the specialists:

- `policy-bench replay --recheck true --ask DECK` asks the deck's specialist at
  every decision of a generic game. The specialist follows the duel as
  played, and the replay tells where it would have answered otherwise.
- `policy-bench generic --plain true --borrow KINDS` makes the generic play the
  specialist's answer on the chosen decision kinds. `Idle:ACTION` names a
  Main Phase decision by what the specialist does in it. Borrowing every
  decision plays like the specialist (49.5% on the 19 decks of the headroom
  table, against 31.5% for the generic alone), so each kind's gain is its
  share of the gap.

The Main Phase carried most of the gap: borrowed alone, it gave +11.6 of the
18 points, against +1.1 for chain windows. The disagreements the shadow
specialist counts most often were not the ones that cost games. Skill Drain,
for instance, does nothing in the Exodia mirror, whose monsters are Normal
Monsters. Only borrowing measured the cost.

Every rule below reads the card's printed type and its effect descriptors,
never a deck name. The specialists are unchanged. Plain games, all 71 decks,
128 games each (seed 3100000, the same deals for every version):

| Change | Win rate | Paired difference | Largest changes (wins of 128) |
| --- | ---: | ---: | --- |
| Committed pilot | 36.74% | | |
| Set Continuous cards turned face-up (attack locks, Skill Drain, Pyramid of Light, Morphtronic Bind, Ojama Trio); Waboku and Threatening Roar used freely with no attacker of ours | 36.62% | −0.12 [−0.34, +0.10] | Destiny Board +4, Pyramid −3 |
| A Set Flip monster is Flip Summoned when its Flip effect has something to do, unless the opponent could then attack it for lethal damage | 37.87% | +1.25 [+0.90, +1.61] | Pyramid +37, Deck-out +23, Nordic +15, Gravekeeper +8 |
| A pure hand trap (ATK and DEF 300 or less, a Quick or Trigger effect used from the hand) is never Summoned | 38.12% | +0.24 [+0.04, +0.45] | Chain Burn +31 |
| A weak Effect Monster with no effect that needs it face-up is Set, Tuners excepted; Special Summon picks favour what the monster does as it arrives (not for Fusion or Ritual Monsters); Gravity Bind is used alongside Messenger of Peace, and Reload as the Exodia specialist uses it | 38.58% | +0.46 [+0.19, +0.73] | Gladiator +12, Reptilianne +12, Exodia +5, Venom +5 |

Measured and dropped:
- **Toon Defense:** its optional trigger, used blindly, turns attacks on our
  Toons into direct attacks (Toon 17 → 11 of 32 with the search).
- **Summoning, not Setting, any monster with an effect that needs it face-up:**
  +289 and −288 games, with Arcana, Gladiator and Reptilianne down by 10 or more.
- **Every specialist's return of Honest to the hand:** −0.06.
- **Holding Secret Barrel and Magical Explosion for a bigger turn:** −1 game.

With the staged search, on the deals of the roster run (32 games a deck), the
19 decks of the headroom table:

| Pilot | Win rate | Decks at 18 of 32 or more |
| --- | ---: | ---: |
| Committed | 47.2% | 0 |
| After the hand-trap rule | 50.3% | 5 |
| **After the Summon and pick rules** | **54.3%** | 9 |
| Specialist (headroom) | 69.7% | 18 |

The last pilot against the committed one: +7.1 [+3.7, +10.5] points (78
games gained, 35 lost). Gladiator went from 17 to 25, Volcanic and Reptilianne
from 15 and 17 to 22, Chain Burn from 11 to 18. Exodia (11) and Machina (13)
did not move. Plain gains do not carry over one for one: the search had
already been finding Pyramid's Flip Summons, so Pyramid's +37 of 128 in plain
play was +1 of 32 with the search.

The whole roster with the last pilot, on the deals of the roster run (the 19
decks above and the other 52 in a second run, `pilot/`):

| Arm | Wins (70 decks) | Rate | Decks at 55% or more | Wilson lower bound at 55% | Wilson upper bound under 55% |
| --- | ---: | ---: | ---: | ---: | ---: |
| Staged search, committed pilot | 1405/2240 | 62.7% | 51 | 13 | 3 |
| **Staged search, last pilot** | 1463/2240 | **65.3%** | 57 | 18 | 1 |

Paired over the 2,240 required games: +2.6 [+0.9, +4.3] points (219 gained,
161 lost); Countdown 17 of 32 (was 18). Still under 55%, wins of 32:

- Exodia 11 (the only one whose interval is all under 55%), Machina 13,
  Gravekeeper 14, Harpie 14;
- Benkei 15, Deck-out 15, Burn 16, Destiny Board 16;
- Batteryman 17, Chaos 17, Dragunity 17, Pyramid 17, Six Samurai 17.

Some decks lost games: Chaos fell from 23 to 17 (3 gained, 9 lost), Naturia
from 25 to 20, Garden from 26 to 22. At 32 games a deck these are within
noise one by one; Chaos is the one to look at first.

## Cost

Single-threaded, a searched decision of the final setting takes 0.34 s at the
median and 3.62 s at the 95th percentile; a game with search costs about
61 s of CPU. Plain games take about 70 ms. Every number here comes from 16
workers on CPUs 8-23 of this host. Spreading each stage's worlds over threads
would divide the latency roughly by the thread count (not built). The search is
deterministic: no time budget, the same results under any load.

## What is still missing

- **The rest of the pilot's gap.** Borrowing every decision of the
  specialists still adds 12.5 points of plain play on the 19 weakest decks
  (37.1% → 49.6%), most of it in the Main Phase. It is now archetype play
  rather than card timing: Six Samurai's summon chains, Morphtronics that work
  in Attack Position, Harpie's chain windows, Exodia keeping its Spell & Trap
  Zones for Heart of the Underdog rather than a second lock. The simple
  card-fact rules tried after the last pilot measured flat or worse (above).
  Archetype rules would be card knowledge too, but they help unknown decks
  little.
- **Tribute Sets.** Pyramid's Level 5 and 6 Sphinxes are Tribute Set: their
  roots stay undealt (638 in `fd2-resample`; about 11 a game for Pyramid and 12
  for Monarch in `roster/best`). Dealing them needs the Level range the Tributes
  allow.
- **Latency.** Parallel worlds, above.
- **The target.** 32 games a deck measure a win rate to about ±9 points; a
  per-deck 55% needs a statistical criterion and about 200 games a deck.

## Reproduce

Build the candidate (`cargo build --release -p ygo-policies-ffi -p ygo-policies-bench`)
and the frozen specialists from `7ddad06` the same way, then

```sh
BASELINE=/path/to/7ddad06/libygo_policies.so benchmarks/generic-policy-v3/run.sh staged final.jsonl
BASELINE=... benchmarks/generic-policy-v3/run.sh staged roster.jsonl all 2700000
python3 benchmarks/generic-policy-v3/compare.py hard16/plain.jsonl.gz final.jsonl
python3 benchmarks/generic-policy-v3/roster.py roster/plain.jsonl.gz roster/mcts32.jsonl.gz roster/best.jsonl.gz
```

The raw rows and metadata (`hard16/`, `roster/`, `headroom/`, `pilot/`) are not in the repository;
`run.sh` regenerates any arm. The arms of `hard16/` other than `final`, and `roster/best`, ran from scratch
copies of the branch with environment switches, which their metadata does not
record. The repository's build reproduces them game for game: plain 512/512,
MCTS 64/64, and `--method staged` 96/96 on the Deck-out, Chain Burn and Agents
games of `roster/best`.

| Arm | How it ran | The same with this repository |
| --- | --- | --- |
| `plain`, `mcts32` | the arguments in its metadata | `run.sh plain`, `run.sh mcts32` |
| `mcts32-terminal` | `mcts32` with `--mcts-terminal true --horizon 0 --mcts-decisions 4096` | the same arguments |
| `staged-generic` | `--method rollout --horizon 0 --millis 0 --worlds 64 --max-candidates 8`, `YGO_STAGED=8,24,64` | `run.sh staged-strict` |
| `staged-large` | as `staged-generic`, `YGO_STAGED=16,48,128 YGO_STAGED_KEPT=3`, `--max-candidates 12` | `--stages 16,48,128 --kept 3 --max-candidates 12`, both switches off |
| `staged-specialist` | as `staged-generic`, `--opponent-model specialist` | `staged-strict` with `--opponent-model specialist` |
| `oracle-facedown` | as `staged-generic`, `YGO_ORACLE_FACEDOWN=1 --strict false` (cheating) | not kept |
| `staged-chains` | as `staged-generic`, `YGO_SEARCH_CHAINS=1` | `--resample-set-monsters false` |
| `fd-resample`, `combined` | first engine version (Normal Sets only), `YGO_RESAMPLE_FACEDOWN=1`, `combined` also `YGO_SEARCH_CHAINS=1` | not kept: the engine now takes face-down Special Summons too |
| `fd2-resample`, `roster/best` | the final engine; `fd2-resample` without chains, `roster/best` with both | `--search-chains false`; `run.sh staged` |
| `final` | `run.sh staged` | |
| `headroom/specialist`, `headroom/specialist-plain` | `run.sh headroom`, `run.sh specialist-plain` with the 19 decks of the table and seed 2700000 | |
| `pilot/hand2-staged` | `run.sh staged`, the 19 decks of the headroom table, seed 2700000, built after the hand-trap rule | |
| `pilot/v3d-staged`, `pilot/v3d-staged-rest` | `run.sh staged`, the same 19 decks and then the other 52, seed 2700000, built from this repository's current pilot | `run.sh staged OUT all 2700000` |
