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

# The complete current roster (29 policies = 103,936 games at 256 per pair).
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

## Protocol and artifacts

- Master Rule 1, 8000 LP, five-card opening hands, one draw per turn (including
  the first turn). `--games` must be positive and even; seats alternate.
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
  checks, not security. `--trace true` includes every decision and chosen response
  for debugging and can produce large files.
- `--markdown PATH` writes a standalone ranking and matrix from a tournament or
  matchup run. Historical editorial commentary is not carried into generated data.

Tests: `cargo test -p ygo-policies-bench`. The tests cover schedule uniqueness,
seat balance, subset-stable seeds, paired uncertainty, rating orientation and
refusal to rank failed/incomplete runs. Engine smoke tests are ordinary CLI runs;
running `compare` with the same library twice should produce identical transcripts.
