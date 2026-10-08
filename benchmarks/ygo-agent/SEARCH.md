# Step 3: the pilot with a search, against the model

[Step 2](README.md#measured-2026-10-08) measured ygo-agent's released model
against the low-effort `blue-eyes` pilot: the model wins 1,659 of 2,000
games, 83.0% ± 0.8%. This step puts the bench's one-step search on top of
the same pilot.

**The question:** does a quick pilot with a search reach a player that had
100 million games of self-play?

**A first look, 2026-10-08, 200 deals: not as the search stands.** With the
search the pilot wins 59 of 200 games against the model where alone it wins
33: 29.5% against 16.5%, 13.0 ± 3.3 points more, and the model still wins
seven games in ten. Against the pilot itself the same search wins 164 of
200.

## Decided on 2026-10-08

- **In the search's try-outs the model's seat is played by a stand-in, the
  pilot itself.** The search plays every alternative out to the end of the
  game, some 2,000 times a game, and the model would have to answer every
  decision of its seat in each: over 100,000 requests for one searched game,
  where a plain game asks 120. The stand-in takes decisions the model would
  not, so the search plans against another opponent than the one it meets
  and may show little for it. That was known and accepted for a first look.
- **A small sample first**, to see where this stands.
- **Only `0546_22750M`.**
- **If the model itself is to play the try-outs**, that run waits for the
  faster GPU of the training machine.

## What was built

`policy-bench search` (see "A search on top of a pilot" in the bench's
README) plays each deal twice, the pilot alone (`baseline`) and the pilot
with a search at each of its decisions (`search`). It now takes the model as
the other player:

```sh
target/release/policy-bench search --rules mr5 \
  --policies blue-eyes --opponents ygo-agent --server http://127.0.0.1:3013 \
  --games 200 --seed 860000 --workers 20 --record true --output OUT.jsonl
```

- **The duel itself.** The searching seat is what `search` makes of it. The
  other seat is answered by the model, every one of its decisions, as
  `matchup` has it answered: the same requests in the same order, one
  session of the server a game.
- **A try-out.** The model's seat is played by the stand-in: a seat of that
  policy, built by name and fed everything the model's seat was fed in the
  duel. A try-out never asks the server and never touches the model's seat.
- **`--stand-in NAME`** names the stand-in; the default is the searching
  policy itself. The row's `search` says which it was (`stand_in`).
- **`baseline`** is the plain game of the pilot against the model on that
  deal, as `matchup` plays it. Both games of a row carry `model_seats` as
  the rows of `matchup` do.
- **The deals are those of `matchup`** for the same pair and `--seed`:
  `--seed 860000` is the deals of `measured/22750M-pilot`.
- **A pair of the same name** among the benchmark's own players is searched
  too: `--policies blue-eyes --opponents blue-eyes` is the pilot with a
  search against the pilot alone, where the stand-in is exactly right.
- **`--validate` and `--foresight`** are refused with the model: both play
  the other player itself out.
- **`kit/report.py`** prints the figures below from the rows.

In the code: `Outside` (the seat that answers and the name of its stand-in)
and `LiveSeat` in `crates/ygo-policies-bench/src/engine`, and
`ygo_agent::seat`. The stand-in sits at the search's table in the model's
place and is fed what the model's seat is fed; what it answers in the duel
itself is dropped. The `blue-eyes` pilot keeps nothing of its own earlier
answers, so what it knows in a try-out is what happened in the duel.

## The checks

| Check | |
| --- | --- |
| A searched game in which no answer was changed is the plain game, answer for answer (and the model was asked as often in both) | 34 of 34 against the model, 24 of 24 against the pilot |
| The `baseline` of a row is the game `matchup` plays on that deal | 200 of 200, against the run of step 2 |
| Try-outs that failed | none of 416,168 against the model, none of 438,072 against the pilot |
| Against the pilot alone the search gains | 164 of 200 where the pilot alone wins 100 |
| An outside seat that answers as its stand-in would changes nothing: the same try-outs, the same answers changed, the same digest | a test of the bench (`an_outside_seat_that_answers_as_its_stand_in_changes_nothing`) |
| A second run gives the same rows | 8 of 8 deals, the plain and the searched game |

## The first look

One server on `0546_22750M`, the first 200 deals of `measured/22750M-pilot`
(`--seed 860000 --games 200`), the search as it is by default (8, 32 and 96
worlds, `--z 1.645`).

| The pilot | Alone | With the search | Deal by deal |
| --- | ---: | ---: | --- |
| **against the model** (stand-in: the pilot) | 33 of 200, 16.5% ± 2.6% | **59 of 200, 29.5% ± 3.2%** | +13.0 ± 3.3 points: 36 deals turned its way, 10 against |
| against the pilot (the stand-in is exact) | 100 of 200, 50.0% ± 3.5% | 164 of 200, 82.0% ± 2.7% | +32.0 ± 3.7 points: 70 deals turned its way, 6 against |

| The pilot, by seat | Going first | Going second |
| --- | ---: | ---: |
| against the model, alone | 17 of 100 | 16 of 100 |
| against the model, with the search | 36 of 100 | 23 of 100 |
| against the pilot, alone | 42 of 100 | 58 of 100 |
| against the pilot, with the search | 77 of 100 | 87 of 100 |

**What the search did** in a game against the model: 40 decisions searched,
8 left to the pilot because a chain was open and fewer than 1 because of a
face-down monster, 2,081 try-outs, and 2.4 answers changed, 405 of the 489
in the Main Phase. It takes 51 seconds of one core; the 200 deals took 10
minutes on 20. Against the pilot: 50 decisions searched, 2,190 try-outs, 2.5
answers changed, 52 seconds.

**What it says:**

- **The search works for this deck.** Against the pilot it wins 82%, as
  often as the model wins against the same pilot (83.0%).
- **Against the model it gains less than half as much**, and the model
  wins 70.5% ± 3.2% of the games.
- **So two players that beat the pilot equally often are far apart when
  they meet.** Two things would give that, and this run does not tell them
  apart. The search plans against the pilot's answers, which are not the
  model's: the stand-in is exact in the second row and not in the first.
  And to beat this pilot is not to play well: what the search finds may be
  what beats the pilot.
- **Going first the search is worth 19 ± 4 points against the model, going
  second 7 ± 5.** One run of 100 deals a seat: to be seen again before it
  is read.

**What would tell the two apart** is the model itself in the try-outs.
That is some 2,000 try-outs of about 60 requests each for one game, so it
needs a server that answers many requests at once, on the training
machine's GPU, and a copy of the model's memory of the duel for each
try-out. Not built.

**More deals of this run** would narrow the figures and not move them: the
direction is plain at 13.0 ± 3.3, and so is where it leaves the pilot.

## The files

| In `measured/` | |
| --- | --- |
| `search-200-model.jsonl.gz`, `.metadata.json` | the pilot with a search against the model |
| `search-200-pilot.jsonl.gz`, `.metadata.json` | the pilot with a search against the pilot |

The metadata of both names the checkout as `b424d29` with files modified:
they ran before this step was committed. The bench as committed plays the
first 8 deals of the run against the model again with the same digests.
