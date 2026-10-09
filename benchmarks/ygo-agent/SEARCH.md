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
  faster GPU of the training machine. What it needs is built and checked
  since: [The model itself in the try-outs](#the-model-itself-in-the-try-outs).

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

**What would tell the two apart** is the model itself in the try-outs:
[below](#the-model-itself-in-the-try-outs), built and not yet measured.

**More deals of this run** would narrow the figures and not move them: the
direction is plain at 13.0 ± 3.3, and so is where it leaves the pilot.

## The model itself in the try-outs

Prepared on 2026-10-09 and checked on small runs. **Not measured: no figure
below says how the pilot does.**

### What was built

- **A copy of the model's memory for every try-out.** The model's answer
  depends on what it saw earlier in the duel: a state of its own and its
  last 32 actions. Through `POST /v1/predict` the server keeps neither: the
  answer carries them and the seat sends them back with its next request
  (`--carried true`; [The server](README.md#the-server)). A seat that
  carries its duel's memory is copied by copying it.
- **`--stand-in ygo-agent`.** In every try-out a copy of the model's seat as
  it stood at the decision plays the model's seat, each of its decisions a
  request to the server. The memory is then carried in the plain game too,
  so that both games of a row are asked the same way.
- **Many requests at once**: `serve.py --device gpu --batch N --fronts N`.
- **One command**, [`kit/search-model.sh`](kit/search-model.sh): it builds
  the bench, starts the server on the GPU, plays the deals, prints the
  report and stops the server.

**One limit stays.** A try-out deals the hidden cards again. A copy's memory
is of the duel as it was played, so in a try-out the model holds another
hand than the one it remembers drawing; what it is shown at each decision is
the try-out's table.

### What was checked

| | |
| --- | --- |
| The rewritten server answers as the one of steps 2 and 3 did | 2,515 requests of 12 logged games put to it again, through its sessions and with the memory carried: every first choice the same, no probability further off than 1e-16. The bench plays 16 deals of step 2 again with their digests through either way of asking |
| On a GPU a request's answer does not depend on what shares its batch | 96 requests put at other places among other requests: the same to the last bit. 16 deals played twice: the same games. Not so on a CPU, nor between batches of two sizes, which is why every batch has one size |
| The GPU against the CPU | 5 first choices of 2,515 differ, each between options that are tied; 14 of 16 games are the same and all 16 have the same winner |
| Copies of an outside seat in the try-outs change nothing where that seat answers as a policy would | the bench's test `an_outside_seat_that_answers_as_its_stand_in_changes_nothing` |
| The model in the try-outs: 29 deals with a light search (4, 8 and 16 worlds) on a GTX 1060 | 13,580 try-outs, none failed; 1,197,778 requests in them, none unanswered; 16 searched games with no answer changed, all 16 the plain game answer for answer |
| `kit/search-model.sh` from start to end | 4 deals with a search of 2 worlds, and 2 deals on a setup made from nothing with `GPU=1`. An interrupt stops the bench and the server with it; a submodule that is not at its pin stops the script before it builds. The folders may be given from where the script is started (`OUT`, `AGENT`, `VENV`, and the four of `setup.sh`): a run with all of them so, and one with no `OUT`, give the same rows |
| A clone made from nothing | cloned with its submodules, built (71 seconds) and run on 2 deals: the rows of the working copy, answer for answer. The engine is pinned one commit after its fork's `master` (`f1a6ec1`, a larger store for a duel): the clone needs that commit on the fork |

### What it costs

- **A try-out asks the server 88 times** (the 29 deals). With the search of
  the first look (8, 32 and 96 worlds, 2,081 try-outs a game) that is some
  180,000 requests a game and 37 million for 200 deals. With 4, 8 and 16
  worlds a game had 468 try-outs and 41,000 requests, the largest 105,000;
  that search changes 0.55 answers a game where the first look's changes
  2.4, so it is another, weaker search.
- **The try-outs of a deal run one after another.** A deal takes its
  requests times what one request takes, however many deals run beside it,
  and a run is not over before its longest deal is.
- **On a GTX 1060 with 24 processors**, 96 deals at once and batches of 32
  (the script's settings): 2,800 requests a second with 16 fronts, the
  batches 29 full and the model busy 99% of the time; 2,320 a second with 8
  fronts, the batches 19 full. A batch of 29 takes 10.3 ms, some 6 of them
  the model on the card and the rest the handling around it in the server's
  first process: that process is what limits the run there. Beside it a
  request takes 2.1 ms of a processor in the bench and 2.1 ms in the fronts,
  12 processors busy in all. With 32 deals at once: 1,800 a second, 13 to
  17 ms a request.
- **From these**, on that machine, reckoned and not run: the 200 deals with
  the search of the first look ask 37 million times (416,168 try-outs with
  the pilot standing in), which is 3.7 hours at 2,800 a second and about
  four and a half to five with the long games left at the end. The largest
  game had 8,176 try-outs, 720,000 requests one after another: two to
  three hours by itself. The first games end after some ten minutes, half
  of them after two hours. With the light search, 8 million requests:
  about an hour.
- **A faster GPU shortens the model's part of a batch only**, 6 ms of 10
  there. The handling and the processors' work stay, so a machine with
  slower processors can be slower with a faster card.

### Running it

On a machine with an NVIDIA GPU, a Rust toolchain and
[uv](https://docs.astral.sh/uv/):

```sh
git clone --recurse-submodules -b ygo-agent-benchmark-work REPOSITORY policies-benchmark
cd policies-benchmark
GPU=1 bash benchmarks/ygo-agent/kit/setup.sh
bash benchmarks/ygo-agent/kit/search-model.sh
```

The third command sets ygo-agent up with JAX's build for CUDA 12 (5 GB
under `/tmp`, a few minutes, most of them downloads); the fourth plays the
first 200 deals
of `measured/22750M-pilot` with the search of the first look and writes the
rows, the server's log and the report to `benchmarks/ygo-agent/runs/search-model`.
What it does is set by the environment, as the script's head says:

| | |
| --- | --- |
| `GAMES=200 SEED=860000` | the deals |
| `WORKERS=96` | deals played at once; a worker mostly waits for the server |
| `BATCH=32` | requests the model answers together. Every batch costs the same, full or not |
| `FRONTS` | processes of the server that take the requests: two thirds of the machine's processors unless it is set |
| `STAND_IN=ygo-agent` | who plays the model's seat in the try-outs: the model itself, or a policy such as `blue-eyes` |
| `WORLDS=8 CONFIRM=32 FINAL=96` | the search's stages. Another search than the first look's needs its own run with the pilot standing in, to be compared with: `policy-bench search` with the same three and without `--stand-in` |

The same deals with the pilot standing in are `measured/search-200-model`.
That run was played on a CPU server, and the model breaks ties between
options it rates alike (two copies of a card) differently there: of the
first 91 deals a GPU run finished, 52 have the same plain game as in it.
For a comparison deal by deal, play the stand-in on the same server, where
the plain games are the same games (4 of 4 in a check), in some twenty
minutes:

```sh
STAND_IN=blue-eyes OUT=benchmarks/ygo-agent/runs/search-standin bash benchmarks/ygo-agent/kit/search-model.sh
```

**How fast a run goes** shows in the server's log, `server.log` beside the
rows, a line a minute:

```
311364 requests put to the model in 10693 batches (29.1 a batch); since the last line 2804 a second, the model busy 99% of the time
```

The requests a second say how long the run takes: divide the requests of
the run (37 million for the 200 deals) by them. A model that is busy all
the time with its batches full is the limit itself: a larger `BATCH`
answers more at once, each request a little later. One that is not busy
waits for the fronts or for the bench: more `FRONTS`, or more `WORKERS`.
One that is busy with batches far from full has fewer deals to serve than
a batch holds, and a smaller `BATCH` answers each sooner: 16 deals at once
with batches of 32 gave 1,050 requests a second on the GTX 1060, 8 in a
batch.
The bench writes a game's row when the game ends and says where it stands
every 16 games. An interrupted run keeps the rows of the games that ended;
it cannot be taken up again where it stopped.

## The files

| In `measured/` | |
| --- | --- |
| `search-200-model.jsonl.gz`, `.metadata.json` | the pilot with a search against the model |
| `search-200-pilot.jsonl.gz`, `.metadata.json` | the pilot with a search against the pilot |

The metadata of both names the checkout as `b424d29` with files modified:
they ran before this step was committed. The bench as committed plays the
first 8 deals of the run against the model again with the same digests.
