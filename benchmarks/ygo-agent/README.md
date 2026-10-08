# The released ygo-agent model against a pilot of ours

[ygo-agent](https://github.com/sbl1996/ygo-agent) trains neural players for
Yu-Gi-Oh! by self-play. Its author says of the checkpoint `0546_22750M` that
it had over 100 million games, "equivalent to training on 32 4090 GPUs for 5
days", and compares it with nothing but its own earlier versions. This
branch measures it against a hand-written pilot, in this repository's engine.

**The question:** what do 100 million games of self-play give? A player
that beats a quickly written pilot of the same deck, or not.

**The plan, one step at a time:**

1. A low-effort pilot for their Blue-Eyes deck. Its bar: it beats a random
   player and a player that always takes the first option.
2. The model against that pilot, with those two players as the reference.
3. Only if the model shows competence there: the same pilot with the search
   on top of it. That is [Step 3](SEARCH.md).

**Who does what.** The model's side is done and described here: how to run
it, a server that shows it what its own environment showed it, and the
figures it reaches in its own environment. What is to be built on this
branch is in [What to build](#what-to-build): the rules, the deck, three
players, and the seat that asks the server. The measurements of step 2 are
then run by whoever set up the model's side.

**Measured on 2026-10-08:** the checks hold, and the model wins 1,659 of
2,000 games against the pilot, 83.0% ± 0.8%. See
[Measured](#measured-2026-10-08). With the search on top the pilot wins
29.5% ± 3.2% of 200 games where alone it wins 16.5%: [Step 3](SEARCH.md).

Everything said of ygo-agent below was read in its source at commit
`26293f8` or measured with it on 2026-10-08; where a file is named without a
repository, it is theirs.

## What the model is

- **The checkpoints** are those of their release `v0.1`: `0546_11300M`,
  `0546_16500M` and `0546_22750M` (`.flax_model`, JAX) after 11.3, 16.5 and
  22.75 billion steps, and `0546_26550M.tflite`. 17 MB each, about 4.3
  million parameters. The README's "over 100M games" is `0546_22750M`:
  that is the one measured.
- **The network** (`ygoai/rl/jax/agent.py`): every card of the table as a
  row of 41 numbers, two layers of attention over the rows, the options (at
  most 24) each with the row of its card, the seat's own last 32 actions,
  and an LSTM of 512 that carries the game. It answers a probability for
  each option and its estimate of the game.
- **The game it was trained on** (`ygoenv/ygoenv/ygopro/ygopro.h`): the
  YGOPro engine (`Fluorohydride/ygopro-core`), Master Rule 5, 8000 Life
  Points, five cards and one draw a turn, a game cut after 1,000 steps. A
  pool of 864 cards (`scripts/code_list.txt`). Its example deck, and the one
  its README's own figures are of, is `assets/deck/BlueEyes.ydk`, played
  against itself.

## Setting it up

```sh
bash benchmarks/ygo-agent/kit/setup.sh
```

A few minutes; it needs git, curl, a C++ compiler, CMake and
[uv](https://docs.astral.sh/uv/). It builds their environment from source
(no binary of theirs is used) and puts everything in `/tmp/ygo-agent`
(source, checkpoints), `/tmp/ygo-agent-build` (xmake and what it builds) and
`/tmp/ygo-agent-venv` (Python 3.11 with the packages of mid-2024), about
1.5 GB; `AGENT`, `BUILD`, `VENV` and `SCRIPTS` move them. It ends with a
check: 64 games against their first-option player, and the golden test
below.

Three things in their repository are not as its README says. The script
deals with each, and they are worth knowing before reading their code:

1. **The latest commit does not work with the released checkpoints.**
   `dbf5142` (2024-08-16) numbers 13,472 cards where the checkpoints know
   864, so a model there is shown other cards than those on the table: it
   wins 54% against random play. The last commit with the list they were
   trained on is `26293f8` (2024-07-30), with the engine, the card database
   and the scripts its Makefile pins.
2. **`scripts/eval.py` gives the model an input it was not trained with**, a
   mask of what is visible, which training and `scripts/battle.py` take out
   (`EnvPreprocess(envs, skip_mask=True)`). Through `eval.py` the 100M-game
   model wins 28 to 42% against random play. `eval_masked.py`, which the
   setup writes, is `eval.py` with that one line.
3. **Their server does not show the model what their environment shows
   it.** See [The server](#the-server).

With the GPU build of the same JAX (`jax[cuda12]==0.4.28`) their scripts run
on a GTX 1060: 3,500 steps a second where the CPU does 550. The server needs
no GPU: 7 ms an answer on the CPU.

## What it does in its own environment

The Blue-Eyes deck against itself, the model's first choice (argmax) at
every decision, 1,024 games each (`eval_masked.py --bot_type random|greedy
--num_episodes 1024 --num_envs 64 --seed 1`):

| Checkpoint | Against random play | Against the first option |
| --- | ---: | ---: |
| `0546_22750M` | 1,023 of 1,024 | 1,022 of 1,024 |
| `0546_16500M` | 1,024 | 1,023 |
| `0546_11300M` | 1,023 | 1,021 |

- A game against either lasts 75 to 80 of the model's decisions; a game of
  the model against itself 195 to 200 decisions of both seats, and the seat
  that goes second wins about half of those (51.2% of 2,048 games; 49.6 and
  55% in two runs of 512).
- **Its own estimate is right.** Against itself, over 512 games and 103,000
  decisions, the seat to move won 3.9% of the time where the model gave it
  under 10%, 48.1% where it gave 40 to 50%, and 96.9% where it gave over
  90%: the whole table is under [The checks](#the-checks).
- Their "greedy" player (`GreedyAI` in `ygopro.h`) is the first option of
  their list at every prompt, and their random player a uniform draw from
  it. The order of that list is in [The options](#the-options).

The three checkpoints against each other, 2,048 games a pair
(`battle.py --num-episodes 2048 --seed 0`), each share good to 1.1 points:

| | wins |
| --- | ---: |
| 22.75 against 16.5 billion steps | 51.5% |
| 22.75 against 11.3 | 53.9% |
| 16.5 against 11.3 | 50.8% |

Twice the training, from 11.3 to 22.75 billion steps, is worth 4 points
against itself, and the seat that goes second wins 51 to 52% in all three.

So the released model is far past both players that the pilot of step 1 has
to beat, and its three checkpoints are close to one another. What any of
them is worth against a pilot is not known.

## The match

- **The deck:** [`kit/BlueEyes.ydk`](kit/BlueEyes.ydk), their list at
  `26293f8`: 40 cards and 14 in the Extra Deck (seven Synchro, six Xyz and
  one Link monster). Both seats play it. The side deck is not used. Every
  card of it is among the model's 864.
- **The rules:** Master Rule 5, which is `DUEL_MODE_MR5` in
  `vendor/ocgcore/ocgapi_constants.h`. 8000 Life Points, a hand of five,
  one draw a turn, and the seat that goes first does not draw. No
  Forbidden & Limited list: the list has three Maxx "C".
- **The engine:** this repository's, with its own scripts and cards. The
  model is asked through the server, which is how their own web client
  asks it.
- **A game** ends as the engine ends it, or at the bench's limit of
  decisions as a draw. The model's seat alternates.

## What to build

1. **Master Rule 5 as an option of a duel**, beside Master Rule 1
   (`DuelOptions` in `crates/ygo-policies-duel`), and wherever the bench
   sets a duel up. Nothing that plays under Master Rule 1 may change: the
   reference games of the pin must come out as they are.
2. **The deck** in `decks/`, loaded as the others are.
3. **Two reference players**, usable wherever a policy is:
   - `random`: a uniform draw among the options, seeded like a pilot's
     ties.
   - `first`: the first option, in the order of [The options](#the-options).
     That order matters: theirs lists summons and activations before the
     change of phase, and declining last, so their first-option player
     summons, sets, activates and attacks whatever it can. A `first` that
     passed whenever passing came first would be a weaker player than the
     one the model's figures above are against.
4. **A pilot for the deck**, `blue-eyes`. Low effort is meant: the bar is
   that it wins clearly more than half of 1,000 games against `random` and
   against `first`, both seats, by the bench's own counting. Say what it
   reaches against each. WindBot has an executor for a close list
   ([`BlueEyesExecutor.cs`](https://github.com/IceYGO/windbot/blob/master/Game/AI/Decks/BlueEyesExecutor.cs),
   deck `AI_BlueEyes.ydk`), which can serve as a first account of what the
   deck does: Trade-In and Cards of Consonance for draws, The Melody of
   Awakening Dragon to search, the White Stones and Sage with Eyes of Blue
   into Blue-Eyes Spirit Dragon and Azure-Eyes Silver Dragon, Blue-Eyes
   Alternative White Dragon's destruction, Rank 8 Xyz.
5. **The model's seat**, `ygo-agent`: a player that describes each of its
   decisions to the server as [The protocol](#the-protocol) says, and plays
   the option the model gives the most. It is the part that decides whether
   the measurement means anything, and [The checks](#the-checks) are how it
   is known to be right. Where the server cannot answer (an `error`), it
   plays what `first` would, and counts it.
6. **The bench**: a matchup between any two of `blue-eyes`, `random`,
   `first` and `ygo-agent` under these rules, both seats, with a row a game
   as the bench writes them, and for the model's seat in each row: how many
   decisions it asked the server, how many the server could not answer and
   why, and the model's own estimate (`win_rate`) at each decision it was
   asked. `--server URL` names the server.

Out of scope here: the search on top of the pilot (step 3), any change to
the pilots of the other decks, and anything of the training repository.

## The server

```sh
cd /tmp/ygo-agent/scripts
/tmp/ygo-agent-venv/bin/python /path/to/benchmarks/ygo-agent/kit/serve.py \
  --checkpoint checkpoints/0546_22750M.flax_model --port 3013
```

[`kit/serve.py`](kit/serve.py) speaks the protocol of their server
(`ygoinf/ygoinf/server.py`) and builds the model's input with that server's
own code (`ygoinf/ygoinf/features.py`). It differs from theirs in three
places where theirs differs from the environment the model was trained in:

| | Their environment | Their server | `serve.py` |
| --- | --- | --- | --- |
| "End Phase" where "Battle Phase" is offered, or "Main Phase 2" | never offered: the turn is ended from the Battle Phase | offered | not put to the model; answered with -1 |
| The times the seat declined (passed a chain, said no) among its earlier actions | left out | kept | left out (`--history-cancel record` keeps them) |
| The identity of a card in its row of the table | none in `YGOPro-v1`, the environment since `04e61b9` (2024-05-21); the one before wrote it | written | none (`--card-ids on` writes it) |

The third makes no difference that can be measured: the model with
identities against the model without won 50.3 ± 1.0% of 2,560 games.

**The golden test** ([`kit/golden_check.py`](kit/golden_check.py)) puts the
first decisions of one game of their environment to `serve.py` as a client
would, and compares what the model is shown, number for number, and what it
answers: all the same. Its four requests and the answers are
[`kit/golden-requests.json`](kit/golden-requests.json) and
[`kit/golden-answers.json`](kit/golden-answers.json): **read them beside the
next section**, they are the reference for every field. `kit/dump_first.py`
writes what their environment shows the model at the first decisions of any
game, with their own log of the game beside it.

**Many requests at once.** By itself the model answers one request at a
time, 4 ms each on a CPU. `--batch N` has it answer the requests that are
waiting together, `--device gpu` puts it on the GPU, and `--fronts N`
starts N processes that read the requests and build its input:

```sh
serve.py --checkpoint checkpoints/0546_22750M.flax_model \
  --device gpu --batch 48 --fronts 8
```

A GTX 1060 answers 128 requests together in 18 ms. Every batch is the same
size, filled up with empty requests, so that on a GPU a request's answer is
the same to the last bit whatever shares its batch; on a CPU it can differ
in the last digit. As the server is started under
[Setting it up](#setting-it-up) it does what it did for every measurement
of this file.

**A duel without a session.** Beside the three calls of the next section
there is `POST /v1/predict`, which keeps nothing of a duel in the server:
the answer carries the duel's memory as `state` (the model's own state and
the seat's earlier actions), and the client sends it back with its next
request, `{"input": .., "prev_action_idx": .., "state": ..}`, with no state
at the duel's first one. An answer that is an `error` carries a state too.
A client can then go on from one point of a duel in several directions,
each with its own copy, which is what a search needs
([Step 3](SEARCH.md)); the bench asks this way with `--carried true`.
[`kit/check_server.py`](kit/check_server.py) sends the requests of games
that were played to a running server again, through either protocol, and
compares the answers.

## The protocol

Three calls, JSON both ways.

- `POST /v0/duels` begins a game for one seat: `{"duelId": "...", "index": 0}`.
- `POST /v0/duels/{duelId}/predict` asks about one decision of that seat:

  ```json
  {"input": {"global": {...}, "cards": [...], "action_msg": {"data": {...}}},
   "prev_action_idx": 4, "index": 1}
  ```

  and is answered

  ```json
  {"predict_results": {"action_preds": [{"prob": 0.39, "response": 201, "can_finish": false}, ...],
                       "win_rate": 0.046},
   "index": 2}
  ```

  or `{"error": "..."}`.
- `DELETE /v0/duels/{duelId}` ends it.

**`index`** is the answer's `index` of the request before (0 at first).
**`prev_action_idx`** is the place, in the answer before, of the option the
seat then played. The server keeps the game's memory by these two: the
model's LSTM state, and the seat's own earlier actions. So:

- **Every decision of the seat goes to the server, in order, also one with
  a single option.** A single option is answered `[1.0]` without the model
  being asked, and still becomes one of the seat's earlier actions, as in
  their environment.
- The decisions of the other seat do not go to it.
- After an `error` the index stays: the seat plays what `first` would and
  sends its next request with the same `index` (its `prev_action_idx` is
  then not read).

**The answer** has one entry for each option sent, in the order sent (and
for a choice of cards one more, see below). `prob` is the model's
probability for it; **-1 means it was not put to the model** and must not be
played for the model: it lies beyond the first 24 options, it is "End Phase"
where the turn is ended otherwise, or it is a card already selected. The
seat plays the entry with the largest `prob`, the first of them if several
are equal. `response` is what the client sent for that option and can be
anything it finds its option by; the place in the list is enough. `win_rate`
is the model's estimate that the seat wins, 0 to 1 (-1 where it was not
asked).

### The table: `global` and `cards`

```json
"global": {"my_lp": 8000, "op_lp": 8000, "turn": 1, "phase": "main1",
           "is_first": true, "is_my_turn": true}
```

`turn` counts both players' turns from 1. `phase` is one of `draw`,
`standby`, `main1`, `battle_start`, `battle_step`, `damage`,
`damage_calculation`, `battle`, `main2`, `end`: the engine's own phase.
`is_first` says whether this seat went first.

`cards` is **every card of both players**, also those the seat cannot see,
since the numbers of cards in each pile are counted from it. The seat's own
first, then the opponent's, each as Deck, hand, Monster Zones, Spell & Trap
Zones, Graveyard, banished, Extra Deck, and within each by its place there.
At most 160.

```json
{"code": 89631139, "location": "hand", "sequence": 0, "controller": "me",
 "position": "facedown", "overlay_sequence": -1,
 "attribute": "light", "race": "dragon", "level": 8, "counter": 0,
 "negated": false, "attack": 3000, "defense": 2500, "types": ["monster", "normal"]}
```

| Field | What it holds |
| --- | --- |
| `code` | The card's passcode, which must be one of the 864. **0 for a card the seat cannot identify.** |
| `location` | `deck`, `hand`, `mzone`, `szone`, `grave`, `removed`, `extra`. The Field Zone and the Pendulum Zones are `szone` at their YGOPro places (see [The two engines](#the-two-engines)). |
| `sequence` | The card's place in its location, **from 0**, as the engine counts it. It ties an option to its card: an option names a card by controller, location and sequence. |
| `controller` | `me` or `opponent`. |
| `position` | On the field, in the Graveyard and banished: as it lies, `faceup_attack`, `facedown_defense`, `faceup_defense`, `facedown_attack`, or `faceup` / `facedown` for a Spell or Trap. The seat's own Deck, hand and Extra Deck: `facedown`. The opponent's Deck, hand and Extra Deck, which the seat cannot see: `none`, and `facedown` after a reveal (below). |
| `overlay_sequence` | -1, or for an Xyz Material its place under the monster, from 0, with the monster's own location and sequence. Materials follow their monster. |
| `attribute`, `race`, `level` | As the card is now. `level` is the Rank of an Xyz and the rating of a Link monster. `none` and 0 for a Spell or Trap. |
| `counter` | Its counters, of the first kind if it has several. |
| `negated` | Whether its effects are negated or it is forbidden. |
| `attack`, `defense` | As they are now. For a Link monster `defense` holds what the card database has in that column, its arrows (Linkuriboh: 2). |
| `types` | Every one that applies, of `monster`, `spell`, `trap`, `normal`, `effect`, `fusion`, `ritual`, `trap_monster`, `spirit`, `union`, `dual`, `tuner`, `synchro`, `token`, `quick_play`, `continuous`, `equip`, `field`, `counter`, `flip`, `toon`, `xyz`, `pendulum`, `special`, `link`. `special` is a monster that cannot be Normal Summoned (Blue-Eyes Alternative White Dragon). |

**What a seat cannot see has nothing but its place.** The opponent's hand,
Deck and Extra Deck, and a face-down card of the opponent on the field:
`code` 0, `attribute` and `race` `none`, every number 0, `types` empty. A
face-down card on the field keeps its `position`.

**A card the opponent revealed is not shown either**, though their code
means to: their environment keeps what was revealed under one name and looks
it up under another, so the model never saw one (none of 9.5 million rows of
the opponent's Deck, hand and Extra Deck in 256,000 decisions of random play
there, `kit/scan_revealed.py`). What a reveal does change is how those rows
are written: from any reveal (the engine's `MSG_CONFIRM_CARDS`, whoever is
shown what) until a chain link next resolves (`MSG_CHAIN_SOLVED`), the
opponent's Deck, hand and Extra Deck have `position` `facedown` in place of
`none`. That is one decision in nine of the model's own games.

**The seat's own Deck is shown with its cards**, as their environment shows
it: the model sees which cards are left in it. A seat does not know their
order, and need not: give each a place of its own, and when the engine asks
for a card from the Deck (The Melody of Awakening Dragon), give the cards it
offers the places it names.

### The options

`action_msg.data` is the prompt, named by `msg_type` after the engine's
message. Their server does not take a prompt outside this list, nor the
cases marked: for those the seat plays what `first` would and counts them.

**`select_idlecmd`**, the Main Phase:

```json
{"msg_type": "select_idlecmd", "idle_cmds": [
  {"cmd_type": "set", "data": {"card_info": {"code": 38120068, "controller": "me", "location": "hand", "sequence": 1},
                               "effect_description": 0, "response": 1}},
  {"cmd_type": "activate", "data": {"card_info": {...}, "effect_description": 0, "response": 5}},
  {"cmd_type": "to_bp"}, {"cmd_type": "to_ep"}]}
```

`cmd_type` is `summon` (Normal Summon, face-up), `sp_summon`, `reposition`,
`mset` (Set a monster), `set` (Set a Spell or Trap), `activate`, `to_bp`,
`to_ep`. **In this order**, each kind in the engine's order: it is the order
of their environment, and the first 24 are all the model is given.

**`select_battlecmd`**, the Battle Phase: `battle_cmds` with `cmd_type`
`activate`, `attack`, `to_m2`, `to_ep`, in this order. `data` as above, with
`"direct_attackable": true` for an attack that would be direct. The target
of an attack is a prompt of its own.

**`select_chain`**, a chance to respond:

```json
{"msg_type": "select_chain", "forced": false, "chains": [
  {"code": 23434538, "location": {"controller": "me", "location": "hand", "sequence": 0, "overlay_sequence": -1},
   "effect_description": 0, "response": 0}]}
```

Unless `forced`, the answer has one more entry at the end: to pass. A chance
with nothing to activate is not a decision and is not sent.

**`select_effectyn`**, whether to use a card's optional effect: `code`,
`location` (as in a chain) and `effect_description`. **`select_yesno`**, any
other yes or no: `effect_description`, which must not be 0. The answer to
both is two entries: yes, then no.

**`select_option`**, one of several things an effect can do: `options`,
each `{"code": DESCRIPTION, "response": N}`.

**`select_position`**: `code` and `positions`, in the order
`faceup_attack`, `facedown_attack`, `faceup_defense`, `facedown_defense`,
those that are allowed.

**`select_place`**, which zone: `"count": 1` and `places`, each
`{"controller": "me", "location": "mzone", "sequence": 2}`, the seat's
Monster Zones first, then its Spell & Trap Zones, then the opponent's. More
than one zone at once is not taken. **The model chooses its zones**: send it
these prompts. `select_disfield` is the same for a zone to make unusable.

**`select_card`**, one or more cards:

```json
{"msg_type": "select_card", "cancelable": false, "min": 1, "max": 2,
 "cards": [{"location": {"controller": "me", "location": "hand", "sequence": 0, "overlay_sequence": -1}, "response": 0}, ...],
 "selected": []}
```

The model takes one card at a time. The answer has an entry for each card,
-1 for those in `selected`, **and a last entry for "no more"**, which is -1
until `min` are selected. Ask again with the place of the card taken added
to `selected` (the places in `cards`, in ascending order) and
`prev_action_idx` its place in the answer, until `max` are selected or the
model takes "no more"; then answer the engine with all of them. `min` 0 is
not taken. **`select_tribute`** is the same, taken only where `min` equals
`max` and every card counts for one Tribute (`"level": 1` on each).

**`select_unselect_card`**, the engine's own one-at-a-time choice:
`selectable_cards` and `selected_cards` (each a `location` and a
`response`), `min`, `max`, `finishable`. One entry a selectable card, and a
last one to finish where `finishable`.

**`select_sum`**, cards whose Levels add up (a Synchro Summon, a Ritual):
`level_sum`, `min`, `max`, `overflow` (true is not taken), `cards` and
`must_cards` (each a `location`, `level1`, `level2` and `response`),
`selected`. One at a time as for cards; `can_finish` in the answer says
that the sum is reached with that card.

**`announce_attrib`**: `"count": 1` and `attributes`, each
`{"attribute": "dark", "response": N}`. **`announce_number`**: `"count": 1`
and `numbers`, each `{"number": 4, "response": N}`, from 1 to 12.

**Not in their list at all**: the order of cards or of a chain, counters to
remove, a declared card name or Type, and who goes first. The seat answers
these itself, as `first` would.

**`effect_description`** tells one effect of a card from another, and
their numbers are YGOPro's:

- 0: the card's only or first effect.
- under 10000: a text of the game itself ("Activate?"). Only these are
  known to the model: 1, 30, 31, 80, 81, 90 to 98, 200, 203, 210, 218 to
  222, 1050 to 1052, 1054 to 1064, 1066 to 1076, 1080, 1081, 1150 to 1169,
  1190 to 1193, 1621, 1622. Another is an `error`.
- else `code * 16 + n`, the card's `n`th text, `n` from 0 to 13.

## The two engines

The model was trained on YGOPro's engine and scripts; this repository runs
EDOPro's. The rules are the same and the cards do the same; the prompts are
not always. What is known to differ:

- **Descriptions.** EDOPro packs a card's text as `code << 20 | n`
  (`aux.Stringid` in `vendor/CardScripts/utility.lua`), YGOPro as
  `code * 16 + n`. Convert every description that is not a text of the game.
  The two sets of scripts number the effects of this deck's cards alike,
  with two exceptions. Phoenix Wing Wind Blast's effect has no description
  in YGOPro's script and the card's first text in EDOPro's: it is sent as 0.
  Number 46: Dragluon has three effects in YGOPro's and one with a choice of
  three in EDOPro's, which cannot be made alike.
- **The summoning procedures** are other texts of the game in the two, and
  are asked about where a monster can be Summoned in two ways: Synchro, Xyz
  and Link Summon are 1172 to 1174 in EDOPro and 1164 to 1166 in YGOPro,
  Fusion Summon 1170 and 1169, Ritual Summon 1171 and 1168. The seat sends
  YGOPro's. The other texts of the game seen in these games (30, 96, 221)
  are the same in both.
- **Zones.** In YGOPro the Field Zone is Spell & Trap Zone 5 (from 0), and
  under Master Rule 5 the Pendulum Zones are Spell & Trap Zones 0 and 4.
  The Extra Monster Zones are Monster Zones 5 and 6 in both.
- **Cards outside the 864.** A token, or a card named by an effect that is
  not in their list, is an `error` for the whole request, since the table
  cannot be built. If one turns up in this deck against itself, say which.
- **Which prompt.** EDOPro asks for cards one at a time
  (`select_unselect_card`) in places where YGOPro asks for them at once
  (`select_card`, `select_tribute`, `select_sum`). Send the prompt the
  engine sent: the model knows both.
- **Windows.** The two engines do not open exactly the same chances to
  respond, nor always in the same order. Nothing can be done about that
  here; it is one of the things [The checks](#the-checks) are for.

When in doubt about what their environment would show, their code says it:
`_set_obs_cards`, `_set_obs_card_`, `_set_obs_global` and the handler of
each message in `ygopro.h`, and `get_legal_actions`, `encode_card` and
`encode_action` in `features.py`.

## The checks

A translation that is a little wrong does not fail: it makes the model play
a little worse, and the measurement then says something false about it.
These must hold before a result against the pilot is read. Each is a
matchup of the bench; 1,000 games, both seats.

1. **Against `random` and against `first`, the model wins as it does in its
   own environment**: 99.8% or more against each there (1,022 and 1,023 of
   1,024). Under 98% here means the translation costs it games.
2. **Against itself it wins half**, the two seats asked through two games
   of the server, and the seat that goes second wins 48 to 56%.
3. **Its own estimate is as right as in its own environment.** Over the
   model's decisions in the games of check 2, set its `win_rate` against
   how those games ended for that seat, in ten bands. In its own
   environment (`kit/native_values.py`, 512 games,
   [`kit/native-values-22750M.json`](kit/native-values-22750M.json)):

   | The model gives the seat to move | Decisions | The seat won |
   | --- | ---: | ---: |
   | under 10% | 7,149 | 3.9% |
   | 10 to 20% | 5,957 | 13.0% |
   | 20 to 30% | 6,857 | 25.8% |
   | 30 to 40% | 9,532 | 37.2% |
   | 40 to 50% | 10,798 | 48.1% |
   | 50 to 60% | 11,769 | 57.0% |
   | 60 to 70% | 11,803 | 67.7% |
   | 70 to 80% | 10,317 | 78.0% |
   | 80 to 90% | 10,412 | 87.6% |
   | over 90% | 18,481 | 96.9% |

   A model that is shown something it does not expect loses this before it
   loses games: a band more than 5 points off its row here, on 2,000
   decisions or more, is a failure.
4. **The server answers nearly always**: `error` at under 1% of the
   decisions sent, and none of a kind that recurs every game. List them by
   kind.
5. **Nothing under Master Rule 1 changed**: matchups of a fixed seed
   between the existing policies give the same rows before and after,
   answer for answer and digest for digest.

If check 1 or 2 fails, find the first decision at which the model's answer
looks wrong (it ends its turn with plays in hand, it passes every chain),
print the request, and read it against what `kit/dump_first.py` shows of a
like position in their environment.

## The measurement

Run once the pilot meets its bar and the checks hold, by whoever set up the
model's side, with the server on `0546_22750M`:

| Matchup | Games | What it is for |
| --- | ---: | --- |
| `blue-eyes` against `random`, and against `first` | 1,000 each | the pilot's bar |
| `ygo-agent` against `random`, and against `first` | 1,000 each | check 1, and the model's reference here |
| `ygo-agent` against `ygo-agent` | 1,000 | checks 2 and 3 |
| **`ygo-agent` against `blue-eyes`** | 2,000 | the result |
| the same with `0546_16500M` and `0546_11300M` | 1,000 each | what the games between 11 and 23 billion steps bought |

Both seats in every matchup, the same deals for every checkpoint. The
result is the model's share of the games against the pilot with its
standard error, by seat, with the draws apart.

It reads one of three ways. The model wins nearly all: 100M games give a
player well past a quick pilot, and the search on that pilot is the next
rung. The pilot wins most: they do not give one. In between: say by how
much, and the next rung decides.

## Implementation and handoff, 2026-10-08

The six items under **What to build** are implemented on branch
**ygo-agent-benchmark-work**. The requested training-repository path was absent;
these instructions and the kit were found in the benchmark checkout at
/tmp/ygo-policies-ygo-agent (commit 6304619) and copied here.

**--rules mr5** selects DUEL_MODE_MR5, including no opening draw for the
first seat. [decks/BlueEyes.ydk](../../decks/BlueEyes.ydk) is the kit's exact
40-card main deck and 14-card Extra Deck. The three native benchmark players
are separate from the ordinary policy roster. The model is a benchmark seat
selected with **--server http://HOST:PORT**; mirrors use independent sessions.

The small pilot uses the draw and search spells, the White Stones and Sage,
Alternative's removal, Spirit/Azure-Eyes, and Rank 8 monsters, with the
existing battle and target helpers. No search was added.

The pilot passed its bar, with 1,000 games against each reference, alternating
seats, seed 830000:

| Reference | Pilot wins | Pilot going first | Pilot going second | Draws / failures |
| --- | ---: | ---: | ---: | ---: |
| random | 1,000 / 1,000 | 500 / 500 | 500 / 500 | 0 / 0 |
| first | 919 / 1,000 | 443 / 500 | 476 / 500 | 0 / 0 |

Evidence: [game rows](pilot-reference.jsonl.gz),
[run metadata](pilot-reference.metadata.json),
[summary](pilot-reference.summary.json).

Adapter tests cover the golden option order and response bytes, visible table
fields, hidden opponent cards, deck inventory without deck order, original
selection indices, masked probabilities, first-tie argmax, single-option
requests, and recurrent index/history after a server error. The supplied
Python golden check also passed. A further 24 integration games (16 against
the references and 8 mirrors) made 3,240 requests with zero server errors,
fallbacks, or engine failures. These small runs establish integration only;
the full model validity measurements remain to be run.

Evidence: [reference smoke summary](adapter-reference-smoke.summary.json),
[reference rows](adapter-reference-smoke.jsonl.gz),
[mirror smoke summary](adapter-mirror-smoke.summary.json),
[mirror rows](adapter-mirror-smoke.jsonl.gz).
Master Rule 1 parity games were omitted at the user's direction.

The model's value head can return a win_rate slightly above 1 (1.0046187 was
observed). Rows preserve the raw number; only the calibration bin is clamped.
A single-option response can have win_rate -1 because the server skips model
inference; a failed request has null. Both remain aligned with the request
count, and neither enters calibration. **--trace true** saves request/response
details; **--record true** saves replay records. A transport failure or malformed
success response fails the game because its recurrent session state is unknown.

Build and reproduce the pilot measurement:

~~~sh
cargo build --release -p ygo-policies-bench -p ygo-policies-ffi
target/release/policy-bench matchup --rules mr5 \
  --policies blue-eyes --opponents random,first \
  --games 1000 --seed 830000 --workers 4 \
  --output /tmp/blue-eyes-reference.jsonl
~~~

For the model-side owner: start the supplied server with checkpoint
0546_22750M as under **The server**, then run the full checks:

~~~sh
target/release/policy-bench matchup --rules mr5 \
  --policies ygo-agent --opponents random,first \
  --server http://127.0.0.1:3013 --games 1000 --seed 850000 --workers 4 \
  --record true --output /tmp/22750M-references.jsonl
target/release/policy-bench matchup --rules mr5 \
  --policies ygo-agent --opponents ygo-agent \
  --server http://127.0.0.1:3013 --games 1000 --seed 850000 --workers 4 \
  --record true --output /tmp/22750M-mirror.jsonl
python3 benchmarks/ygo-agent/kit/check_checks.py \
  /tmp/22750M-references.summary.json /tmp/22750M-mirror.summary.json
~~~

The checker tests sample counts and seats, the 98% reference bar, the
48–56% second-seat mirror bar, calibration within five percentage points in
bands with at least 2,000 decisions, server errors below 1%, and fallback
kinds that recur every game. It omits MR1 parity. Also inspect named-player
mirror wins and draws as requested in **The checks**. If a check fails, trace
the offending seed and compare it with the kit's dump_first.py before using
the result as a strength measurement.

After the checks pass, run the latest checkpoint against the pilot:

~~~sh
target/release/policy-bench matchup --rules mr5 \
  --policies ygo-agent --opponents blue-eyes \
  --server http://127.0.0.1:3013 --games 2000 --seed 860000 --workers 4 \
  --record true --output /tmp/22750M-pilot.jsonl
~~~

Repeat with the 0546_16500M and 0546_11300M servers, **--games 1000**,
the same seed 860000, and fresh output paths. The checkpoint name does not
enter the deal seed. Record each checkpoint's hash and server command with
the result; the HTTP endpoint does not identify the checkpoint. Existing
nonempty outputs are rejected to prevent mixing runs.

Summary wins and by_seat counts refer to player **a**, named in each matchup.
If a is the reference or pilot, its losses are model wins. Draws contribute
half to score but not to win_share. The **rank --input PATH** command can
regenerate benchmark summaries from uncompressed game rows with their adjacent
metadata JSON; decompress the archived rows first.

## Measured, 2026-10-08

By the side that set up the model: `kit/serve.py` with its defaults, one
server a run, and the bench of this branch with the commands of the section
above. No game failed and none ended without a winner. The rows, settings
and summaries of every run are in [`measured/`](measured/), and
`kit/report.py` prints the figures below from the rows.

### Two changes to the seat before the runs

Reading the seat against their environment found two places where the model
was not shown what that environment showed it. Both come from these
instructions and not from how they were followed, and both are changed in
the commit that adds this section.

- **Revealed cards.** The instructions said that a card the opponent
  revealed is shown. Their environment never shows one
  ([The table](#the-table-global-and-cards)). The seat showed whatever lay
  at the revealed place until a chain link resolved, which after the hand is
  shuffled is another card of the opponent's hand. Nothing is shown now, and
  after a reveal the opponent's Deck, hand and Extra Deck are written
  face-down, as there.
- **Phoenix Wing Wind Blast** is sent with the description the model knows
  it by ([The two engines](#the-two-engines)).

It makes little difference. On the same 1,000 deals against the pilot the
seat as committed at `9e595de` won 848 and the changed one 841: 0.7 ± 0.5
points apart, the same result in 971 deals.

### The checks

`0546_22750M`, 1,000 games a matchup (`kit/check_checks.py` passes).

| Check | Here | In its own environment | |
| --- | ---: | ---: | --- |
| 1. Against `random` | 1,000 of 1,000 | 1,023 of 1,024 | holds |
| 1. Against `first` | 992 of 1,000 | 1,022 of 1,024 | holds: 98% asked |
| 2. Against itself, the named seat | 505 of 1,000 | | holds |
| 2. Against itself, the seat that goes second | 51.9% | 51 to 52% | holds: 48 to 56% asked |
| 3. Its estimate, the band furthest from its row | 3.3 points | | holds: 5 asked |
| 4. Requests the server could not answer, all runs of this section | 46 of 862,195 | | holds: 1% asked |
| 5. Nothing under Master Rule 1 changed | not run, at the user's direction | | |

- **Check 1.** It loses 8 games in 1,000 to `first` here and 2 in 1,024
  there, a difference chance gives one time in ten.
- **Check 3**, over the 200,934 decisions of the games against itself:

  | The model gives the seat to move | Decisions | The seat won | In its own environment |
  | --- | ---: | ---: | ---: |
  | under 10% | 13,639 | 4.0% | 3.9% |
  | 10 to 20% | 12,498 | 16.1% | 13.0% |
  | 20 to 30% | 13,978 | 26.5% | 25.8% |
  | 30 to 40% | 18,239 | 36.3% | 37.2% |
  | 40 to 50% | 22,050 | 48.2% | 48.1% |
  | 50 to 60% | 23,665 | 60.3% | 57.0% |
  | 60 to 70% | 22,057 | 68.9% | 67.7% |
  | 70 to 80% | 19,825 | 77.8% | 78.0% |
  | 80 to 90% | 20,727 | 86.5% | 87.6% |
  | over 90% | 34,256 | 98.1% | 96.9% |

- **Check 4.** All 46 are one prompt: which of two ways to Xyz Summon a
  monster, where "Xyz Summon" is text 1173 of the game in EDOPro and 1165
  in YGOPro. The seat took the first way. It sends YGOPro's number since
  ([The two engines](#the-two-engines)); the runs here are from before.

**What the model is asked here is what it is asked there.** 128 further
games against itself with every request kept, beside 256 games in its own
environment (`kit/native_mix.py`, `kit/native-mix-22750M.json`); decisions
with one option are in neither count:

| | Here | In its own environment |
| --- | ---: | ---: |
| Decisions put to the model, a game | 194.9 | 201.6 |
| A chance to respond (`select_chain`) | 71.7 a game | 76.0 |
| of which the model takes | 11.8% | 12.0% |
| Main Phase (`select_idlecmd`) | 38.4, of 8.0 options | 37.8, of 8.0 |
| Which zone (`select_place`) | 28.3 | 28.9 |
| A choice of cards (`select_card`) | 23.5 | 25.9 |
| Battle position (`select_position`) | 13.6 | 13.8 |
| Battle Phase (`select_battlecmd`) | 9.1 | 9.6 |
| Whether to use an effect (`select_effectyn`) | 6.0 | 6.6 |
| One card at a time (`select_unselect_card`) | 2.8 | 0.5 |
| Cards whose Levels add up (`select_sum`) | 0 | 0.8 |
| Decisions after a reveal | 13.1% | 11.1% |

The one difference is the one [The two engines](#the-two-engines) names:
EDOPro asks for the materials of a Synchro or Xyz Summon one card at a time.

### The result

| Checkpoint | Games | The model wins | Going first | Going second |
| --- | ---: | ---: | ---: | ---: |
| **`0546_22750M`** | 2,000 | **1,659, 83.0% ± 0.8%** | 825 of 1,000 | 834 of 1,000 |
| `0546_22750M`, the first 1,000 deals | 1,000 | 841, 84.1% ± 1.2% | 423 of 500 | 418 of 500 |
| `0546_16500M`, the same deals | 1,000 | 852, 85.2% ± 1.1% | 424 of 500 | 428 of 500 |
| `0546_11300M`, the same deals | 1,000 | 803, 80.3% ± 1.3% | 388 of 500 | 415 of 500 |

- **The model is well past the pilot, and the pilot takes one game in
  six.** The same pilot beats `first` 919 times in 1,000 and `random` every
  time (the handoff's figures, run again with the same result); the model
  beats `first` 992 times.
- **Deal by deal**, 16.5 billion steps are worth 4.9 ± 1.2 points over 11.3
  billion against the pilot, and 22.75 billion nothing over 16.5 (-1.1 ±
  1.3). In their own environment a later checkpoint wins 50.8 to 53.9% of
  its games against an earlier one.
- **Games are short**: 6 turns at the median. Those the model wins last 6.4
  turns on average, those the pilot wins 8.2.
- **Against the pilot the model underrates itself.** Where it gives itself
  40 to 50% it wins 79% of the time, and 32% where it gives itself under
  10%: its estimate is of a game against a player like itself.

Of the three ways [The measurement](#the-measurement) says this can read, it
is the third, near the first: 100 million games give a player well past a
quick pilot, which still takes one game in six from it. Whether the same
pilot with a search on top reaches it is the next rung.

### Running it again

One server a run, each on its own port, and the commands of the section
above with `--output` in a folder of the run's own. Five servers at once
took 25 minutes on 24 cores; a server answers about 160 requests a second
and a game against the pilot asks 117.

| Run in `measured/` | Checkpoint (SHA-256 begins) | Opponents | Games | Seed |
| --- | --- | --- | ---: | ---: |
| `22750M-references` | `0546_22750M` (`48e4d3bd`) | `random`, `first` | 1,000 each | 850000 |
| `22750M-mirror` | the same | `ygo-agent` | 1,000 | 850000 |
| `22750M-pilot` | the same | `blue-eyes` | 2,000 | 860000 |
| `16500M-pilot` | `0546_16500M` (`975e673a`) | `blue-eyes` | 1,000 | 860000 |
| `11300M-pilot` | `0546_11300M` (`25035d28`) | `blue-eyes` | 1,000 | 860000 |
| `22750M-pilot-as-committed` | `0546_22750M`, the bench built at `9e595de` | `blue-eyes` | 1,000 | 860000 |

- The metadata of these runs names the checkout as `9e595de` with
  `ygo_agent.rs` modified: they ran before the change to the seat was
  committed. The last run is of a bench built before that change.
- **Single games are not reproduced to the last answer on another server
  setting.** A server held to one thread and one left to use several differ
  in the last digit of a probability; where two options are tied (two
  copies of a card in hand) the other one is then taken, and the game goes
  another way from there. The results agree.

## The kit

| File | What it is |
| --- | --- |
| [`kit/setup.sh`](kit/setup.sh) | ygo-agent at `26293f8`, built from source, with its cards, scripts, checkpoints and Python environment. |
| [`kit/build-26293f8.diff`](kit/build-26293f8.diff) | What the build needs on a current Linux: its own Lua 5.4.6, one include, six constructor calls. Nothing of what the environment does. |
| [`kit/serve.py`](kit/serve.py) | The server. |
| [`kit/check_server.py`](kit/check_server.py) | The requests of games that were played, put to a running server again and its answers compared. |
| [`kit/search-model.sh`](kit/search-model.sh) | The pilot with a search against the model, the model itself in the try-outs: server, run and report in one ([Step 3](SEARCH.md)). |
| [`kit/golden_check.py`](kit/golden_check.py), [`golden.npz`](kit/golden.npz) | The golden test, and what their environment showed and answered in that game. |
| [`kit/golden-requests.json`](kit/golden-requests.json), [`golden-answers.json`](kit/golden-answers.json) | Four requests in a row and their answers. |
| [`kit/dump_first.py`](kit/dump_first.py) | What their environment shows the model at the first decisions of a game. |
| [`kit/native_values.py`](kit/native_values.py), [`native-values-22750M.json`](kit/native-values-22750M.json) | The model's own estimate against how its games end, in its own environment. |
| [`kit/native_mix.py`](kit/native_mix.py), [`native-mix-22750M.json`](kit/native-mix-22750M.json) | What the model is asked in its own environment: how often, of which kind, with how many options. |
| [`kit/scan_revealed.py`](kit/scan_revealed.py) | What their environment shows a seat of the opponent's hidden cards, and after a reveal. |
| [`kit/check_checks.py`](kit/check_checks.py) | Checks 1 to 4 from the summaries of the two runs they need. |
| [`kit/report.py`](kit/report.py) | The results from the rows: the model's share by seat, and two runs deal by deal. |
| [`kit/BlueEyes.ydk`](kit/BlueEyes.ydk) | The deck. |
