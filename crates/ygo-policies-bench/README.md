# policy-bench

Native OCGCore benchmarks for any registered policy and its deck. This crate builds
the pinned `vendor/ocgcore` and its Lua submodule; `vendor/CardScripts` and
`vendor/BabelCdb` supply scripts and printed card data. A C++17 compiler is required.
The regular policy libraries remain engine-independent.

```bash
git submodule update --init --recursive
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench --help
```

## Matchups and tournaments

```bash
# One policy against selected opponents, or use --opponents existing / all.
target/release/policy-bench matchup \
  --policies arcana --opponents blackwing,monarch --games 256 \
  --output arcana-matchups.jsonl

# A selected pool: every unordered pair exactly once, 256 games per pair.
target/release/policy-bench round-robin \
  --policies arcana,blackwing,monarch --games 256 \
  --output small-tournament.jsonl --markdown small-ranking.md

# The complete current roster (31 policies = 119,040 games at 256 per pair).
target/release/policy-bench round-robin \
  --policies all --games 256 --workers 6 \
  --output tournament.jsonl --markdown DECK-TIER-LIST.md

# Rebuild a ranking from saved results, without replaying any games.
target/release/policy-bench rank \
  --input tournament.jsonl --markdown DECK-TIER-LIST.md
```

The default library is `target/release/libygo_policies.so` (the platform's shared
library suffix is used). `--library PATH` selects another build. Policy/deck names
come from that library's catalog. `--policies` and `--opponents` accept `all`,
`existing` (the original 13 policies), or comma-separated policy IDs.

Ranks use a Bradley–Terry fit centered at 1500 within the measured pool, with a
half-win prior on each observed matchup to keep undefeated results finite. The
report includes win rates, Wilson 95% intervals, sample counts and the full matrix.
Unplayed matchups are left blank. Tiers use the historical 44.7% / 64.6% cuts
against `--reference existing`; a policy is unrated until every required reference
opponent has been played. A small sample still produces a noisy ranking—use the
intervals and counts. `--reference` also accepts an explicit pool or `all`.

### One player first, uneven Life Points

A game can give one side the first turn every game and different starting Life
Points, as *Sands of the Duel* does (the player always goes first at 8000; the
opponent starts at 4000, 6000 or 8000):

```bash
target/release/policy-bench matchup --policies blackwing --opponents all --games 1024 \
  --first blackwing --lp 8000,4000 --output blackwing-first-4000.jsonl
```

`--first POLICY` seats that policy first in every game of its pairs (every pair
must include it); `--lp FIRST,SECOND` sets the starting Life Points of the
first and second player. Without them seats alternate and both start at 8000.

## Paired before/after comparisons

Preserve the baseline library before changing a policy, then rebuild the candidate:

```bash
cargo build --release -p ygo-policies-ffi
cp target/release/libygo_policies.so /tmp/policies-before.so
# Make the policy changes, then rebuild.
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench

target/release/policy-bench compare \
  --policies arcana --opponents existing \
  --baseline /tmp/policies-before.so --candidate target/release/libygo_policies.so \
  --games 512 --seed 1900000 --workers 6 --output arcana-comparison.jsonl
```

Each version pilots the same deck against the **baseline opponent**, using the same
starting shuffle, engine seed, seat and policy tie-break seed. The report records
the aggregate and per-matchup change, improved/regressed results, identical response
transcripts and a paired normal-approximation 95% interval. This measures a policy
change, not a simultaneous change to its opponents. Pass several `--policies` to
compare multiple pilots. A comparison cannot be used as a tournament ranking.

## A search on top of a pilot

`search` plays each game twice on the same seed: the pilot alone, and the
pilot with a one-step search at each of its decisions.  What the search gains
is a floor on what a better pilot of the same deck would gain.

```bash
target/release/policy-bench search --policies monarch --opponents existing \
  --games 64 --workers 20 --core /path/to/libocgcore.so --output monarch-search.jsonl
```

At a decision of the searching seat the duel is snapshotted.  Every
alternative its policy listed (Main and Battle Phase actions, optional chain
responses, yes/no, battle positions; two copies of a card in the same pile
count once, and the zone a card goes to is never an alternative) is tried in
worlds where the cards the seat
cannot see are dealt again: its own Deck in another order, the other
player's Deck, hand and Set Spells and Traps among themselves.  Both policies
play each world out, rebuilt from what the live seats were fed and breaking
ties their own way in each world.  All
alternatives share their worlds, so they are compared world by world.  The
seat leaves its pilot's answer only for an alternative ahead of it by `--z`
(paired, default 1.645) after `--final` worlds (96).  `--worlds` (8) and
`--confirm` (32) are the stages at which alternatives that are not ahead are
dropped; one far ahead after `--confirm` is taken there.

What it cannot do, and what it knows that a player would not:

- It needs `--core`: an OCGCore with arena snapshots and a hidden-card swap
  (the `ygo` repository's build of its `ygopro-core` fork).  The pinned
  engine has neither.
- Cards cannot be dealt again while a chain is open: those decisions are the
  pilot's.
- The other player's face-down monsters cannot be dealt again, so a world
  would show what they are.  By default (`--strict true`) every decision
  taken while one is on the field is the pilot's.
- The worlds are drawn from the other player's real cards: the search knows
  their Deck list, as a policy trained against that deck would.
- Selections of cards and zones are the pilot's.
- Every world starts from the engine's random state at the decision: a coin
  or a die lands as it will in the duel.

`--foresight true` is not a player but a mark of what luck leaves: every
alternative is played out in the world as it is, with the real hidden cards
and the draws to come, and the seat leaves its pilot's answer whenever that
one loses and another wins.  The games it still loses could not have been won
by changing any single answer.

`--validate true` searches nothing: at each decision of the seat it plays
the pilot's answer out in the world as it is and checks that the playout
ends as the duel does (`mispredicted` must be 0).  The duel must also keep
the digest of the plain game.

Each row holds both games (`baseline`, `search`), the search's counts
(decisions searched, left to the pilot in a chain or facing a face-down
monster, playouts) and every deviation: the decision, the pilot's answer,
the one taken, its gain over the worlds and its z.

## Protocol and artifacts

- Master Rule 1, 8000 LP (or `--lp`), five-card opening hands, one draw per turn
  (including the first turn). `--games` must be positive and even; seats
  alternate unless `--first` fixes them.
- The host shuffles both main decks with a specified SplitMix64/Fisher–Yates
  algorithm. Seeds depend on the pair's names and `--seed`, not scheduling or
  the order/size of the requested pool. Filtering a tournament preserves those
  games. The two versions in a comparison share their exact seed and order.
- Each policy receives the raw engine messages and standard `MSG_UPDATE_DATA`
  queries before prompts. Its own front end performs redaction. The benchmark
  never supplies private engine state directly to policy decisions.
- Scripts are searched in the root helpers, `official/`, then `pre-errata/` for
  missing names, including Red-Eyes Darkness Metal Dragon. No script is modified.
- The default decision limit is 4096 (`--limit`); reaching it is reported explicitly
  and scores half a win. Script errors, rejected responses, and missing responses
  are failures, never wins or draws. Failed/incomplete runs cannot publish rankings.
- `--core`, `--cards`, `--scripts`, and `--decks` override local inputs. OCGCore's
  C API must match the vendored v11 ABI. An external core can be used to compare
  against a host build, while normal runs need no neighboring repository.
- `--output NAME.jsonl` streams one record per game/pair; existing files are never
  truncated. `NAME.metadata.json` stores the resolved deck/data/library fingerprints,
  source revisions, dirty state, protocol, and planned sample count.
  `NAME.summary.json` contains statistics. Fingerprints use FNV-1a for reproducibility
  checks, not security. Each game records its number of `turns` and the `reason`
  it ended (1: Life Points, 2: deck-out, 16 and up: a card's own win condition).
  `--trace true` includes every decision and chosen response for debugging and
  can produce large files.
- `--markdown PATH` writes a standalone ranking and matrix from a tournament or
  matchup run. Historical editorial commentary is not carried into generated data.

Tests: `cargo test -p ygo-policies-bench`. The tests cover schedule uniqueness,
seat balance, subset-stable seeds, paired uncertainty, rating orientation and
refusal to rank failed/incomplete runs. Engine smoke tests are ordinary CLI runs;
running `compare` with the same library twice should produce identical transcripts.

## Card facts from the engine

The policies' shared card knowledge (`ygo_policies::knowledge`) is generated
here.  `knowledge` stages every monster of a card pool on a plain board, one
mini-duel per situation, and writes down what the engine does:

- it is attacked, in Attack and in Defense Position, three times in a row by a
  plain 5100 ATK monster, then once by other attackers: every other Attribute,
  four other Types, a lower and a higher Level, one barely strong enough and
  one too weak (where one of them fares differently, the Levels or ATK values
  in between are tried too, to find where the difference starts);
- a Spell, a Trap and a monster effect are used on it, twice each: one that
  targets and destroys, one that targets and does something else, one that
  destroys without targeting, and two that have nothing to do with it;
- it attacks a 0/0 monster in Attack and in Defense Position, and, when its
  attack does more than battle, monsters of the other Attributes and a
  4000/4000 one.

Its controller uses every effect of the monster the engine offers.

```bash
cargo build --release -p ygo-policies-bench
target/release/policy-bench knowledge --workers 8 --report data/wc2011-monster-facts.md
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench   # the table is compiled in
target/release/policy-bench knowledge --workers 8 --rust /tmp/check.rs --output /tmp/misses.txt
```

The first run writes `crates/ygo-policies/src/knowledge/pool.rs` and the
listing; the last one, on the rebuilt binary, reports how many of the staged
attacks the shared tactics (`tactics::default_outcome`) call right with that
table, and lists the ones they do not.  For the World Championship 2011 pool
(`data/wc2011.lflist.conf`, 2,444 monsters, about 100,000 mini-duels, half a
minute on 20 workers) the table holds 344 monsters, and the tactics call
99.7% of 74,959 staged attacks right, against 96.7% with an empty table; of
the 2,697 attacks printed stats do not explain, 92.9% against 10.6%.

A fact is written only when a probe showed it.  What the probes cannot show
stays out of the table, and the listing says which cards need a look:

- the monster is placed, not Summoned: no coin toss (the Arcana Force), no
  counters (the B.E.S. ships);
- effects that need particular cards in the hand, Deck or Graveyard, or a
  board state (Birdface, Morphtronic Cameran, Infernity Guardian);
- effects on other monsters of the same side (Morphtronic Boarden, Aurkus);
- monsters that leave a plain board at once (Malefic, Earthbound Immortals).

Those go in `knowledge::facts` by hand.  `--only CODE,CODE` prints the facts
of a few cards without touching the table; `probe` dumps what the engine did.

## Replaying a recorded duel

*Beat Claudi-oh* keeps every finished duel in its SQLite database with what
it takes to play it again: the seed, both Decks as they were loaded and every
answer either player gave.

```bash
# The last duel of the log, or one by id; --output writes it to a file.
target/release/policy-bench replay --input beat-claudio.sqlite --duel 7
# The same, asking the policy as it is built now at each of its decisions.
target/release/policy-bench replay --input beat-claudio.sqlite --duel 7 --recheck true
```

`replay` runs the record through the pinned engine and tells the duel turn by
turn with nothing hidden: both hands and the Set cards at the start of each
turn, every draw, Summon, activation with its targets, negation, attack and
Life Point change, and at the end what each player held when each turn ended.
Where a player could have chained a card and passed, it says so
(`(Claudi-oh could activate Torrential Tribute and does not)`), once per turn
for the same cards.  It reads what a policy did against a player, which the
benchmarks cannot show.

`--recheck true` plays the policy of the duel's row (`--library`, default the
one built here) alongside the record: it is fed the duel as its seat saw it,
asked at each of that seat's decisions, and the duel goes on with the recorded
answer whatever it says.  Where it would answer otherwise the story has a line
starting with `>>`, before what was done:

```text
   Duelist Normal Summons Elemental HERO Neos Alius
   >> as built now, Claudi-oh would pass here, not activate Solemn Warning
   Claudi-oh activates Solemn Warning [the Spell & Trap Zone]
```

That tells whether a change to a policy reaches the position it was made
for.  Each answer is given with the duel as recorded up to there, so the lines
are independent of one another; between cards a policy values the same, the
pick is drawn at random and can differ from the record for no other reason.

The record must come from the same engine, scripts and card database as this
checkout's `vendor/`; a record from other versions stops with an error when an
answer no longer fits the question.  `--input` also takes a `.json` file
holding one duel's `replay` column (without `--recheck`: the bare replay does
not say which policy played).
