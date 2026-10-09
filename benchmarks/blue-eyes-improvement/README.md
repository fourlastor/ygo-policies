# Blue-Eyes: search-guided policy improvement

Measured on 2026-10-09. `blue-eyes-improved` wins **4,859 / 8,192 games
(59.31%)** against the unchanged `blue-eyes` policy. On the identical deals,
seats and policy seeds, the original copy wins 4,105 / 8,192 (50.11%).
The paired improvement is **+9.20 percentage points**, with a normal-approximation
95% interval of **[+8.33, +10.07]**. There are no draws, errors or decision caps
in either version's 8,192 games (16,384 duels total).

Both policies remain selectable and use the unchanged `decks/BlueEyes.ydk`:

- Original: [`blue_eyes.rs`](../../crates/ygo-policies/src/decks/blue_eyes.rs), ID `blue-eyes`.
- New: [`blue_eyes_improved.rs`](../../crates/ygo-policies/src/decks/blue_eyes_improved.rs), ID `blue-eyes-improved`.

Only the new policy's rules changed. Both stay outside the WC2011 `all` roster.
This measures the Blue-Eyes mirror against this fixed opponent, not performance
against other decks or the ygo-agent model.

## Rules retained

1. Use White Stone of Ancients to recover the missing half of the
   Blue-Eyes/Alternative pair before a revival spell consumes that graveyard
   target. Also recover Alternative when Blue-Eyes is already in hand.
2. Special Summon Alternative before draw/search spells can discard the
   Blue-Eyes needed to reveal for its summon.
3. Consider the opponent's battle stat, including DEF, when making Prime
   Photon Dragon. This opens the Prime / Full Armor route against 3,000-DEF
   Azure-Eyes walls that the original's ATK-only check misses.

The search also suggested Normal Summoning Effect Veiler as a tuner with a
Level 8 dragon. Its paired test showed no gain, so that change was removed.
All other shared policy code, card selections and deck lists are unchanged.

## Search and development evidence

The new file began as a byte-for-byte copy. `copy-check` ran 32 pairs: all
response digests were identical. The baseline library was preserved at that
point, including the copied policy ID, so later comparisons keep both the
opponent and the candidate's initial identity fixed.

`search-baseline` searched 32 games with seed 4100000, MR5, eight workers,
8/32/96 worlds, z=1.645, strict hidden-card handling, logs and replay records.
It won 23 games versus the plain pilot's 14, changing 75 decisions across
73,312 playouts with zero failed playouts or capped duels. This is hypothesis
generation; it is not the final win-rate estimate.

Examples from `search-baseline.jsonl.gz` (engine seeds, decision indices):

| Rule | Search evidence |
| --- | --- |
| Recover before revival | 335610598 / 96: Ancients instead of Return; 94 versus 81 wins in 96 worlds |
| Recover Alternative | 335610601 / 210: Ancients instead of ending the turn; 41 versus 25 in 96 |
| Summon before discarding | 335610602 / 529: Alternative instead of Trade-In; 89 versus 72 in 96 |
| Break the DEF wall | 335610613 / 208 and 335610614 / 442: Prime instead of entering battle, facing defense-position Azure-Eyes |
| Rejected Veiler rule | 335610599 / 603 and 335610618 / 526: summon Veiler instead of entering battle |

Each development row compares the proposed addition with the preceding accepted
version, always against the original `blue-eyes`. The opponent's implementation
is unchanged in every stored library. Seats alternate; limits are 8,192 decisions.

| Run | Pairs | Seed | Before | After | Paired change, percentage points [95% CI] |
| --- | ---: | ---: | ---: | ---: | ---: |
| `recovery` | 1,024 | 4300000 | 49.90% | 57.42% | +7.52 [+5.23, +9.81] |
| `summon-first` | 1,024 | 4300000 | 57.42% | 58.98% | +1.56 [-0.55, +3.68] |
| `summon-first-confirm` | 4,096 | 4400000 | 56.62% | 57.96% | +1.34 [+0.37, +2.31] |
| `defense-wall` | 1,024 | 4300000 | 58.98% | 60.84% | +1.86 [+0.99, +2.73] |
| `veiler` (rejected) | 1,024 | 4300000 | 60.84% | 60.74% | -0.10 [-1.13, +0.93] |

These development estimates were used to choose the rules; only the frozen
final policy was evaluated on the held-out sample.

## Held-out validation

`heldout` uses seed 4500000 and six workers. Its 8,192 unique engine seeds do
not overlap any search or development seeds. Within each pair both versions
get the same shuffle, engine seed, seat and policy tie-break seed. The original
opponent always comes from the frozen baseline library.

| Candidate's seat | Pairs | Original wins | Improved wins | Paired gain |
| --- | ---: | ---: | ---: | ---: |
| First | 4,096 | 1,849 | 2,212 | +8.86 points |
| Second | 4,096 | 2,256 | 2,647 | +9.55 points |
| Both | 8,192 | 4,105 | 4,859 | +9.20 points |

Of the 8,192 pairs, 1,074 changed from a loss to a win, 320 from a win to a
loss, and 2,588 retained identical response digests.

- `snapshot-check`: eight games, seed 4600000, two workers, 1,663 predictions,
  zero mispredictions or failed playouts, all eight plain/search digests identical.
- `YGO_CARDS_CDB="$PWD/vendor/BabelCdb/cards.cdb" cargo test --workspace`:
  265 tests passed, none failed or ignored. The HTTP fixture requires localhost
  socket permission; the successful run used it.
- The original source SHA-256 remains
  `41a57727053ef7ff467b9dfb11a156a66abb5c3752d964e56a0ca13ea8385e79`.

## Reproduce

Run from the repository root. Both IDs are registered in the same library:

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench matchup \
  --policies blue-eyes-improved --opponents blue-eyes --rules mr5 \
  --games 8192 --seed 4700000 --workers 6 --limit 8192 \
  --output blue-eyes-new-matchup.jsonl
```

To recreate the exact paired protocol, preserve the final source while building
a library in which the new ID still points to the unchanged copy:

```sh
set -eu
scratch=$(mktemp -d)
candidate=crates/ygo-policies/src/decks/blue_eyes_improved.rs
cp "$candidate" "$scratch/final.rs"
trap 'cp "$scratch/final.rs" "$candidate"' EXIT
cp crates/ygo-policies/src/decks/blue_eyes.rs "$candidate"
cargo build --release -p ygo-policies-ffi
cp target/release/libygo_policies.so "$scratch/baseline.so"
cp "$scratch/final.rs" "$candidate"
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench compare \
  --policies blue-eyes-improved --opponents blue-eyes --rules mr5 \
  --baseline "$scratch/baseline.so" --candidate target/release/libygo_policies.so \
  --games 8192 --seed 4500000 --workers 6 --limit 8192 \
  --output blue-eyes-repeated-pairs.jsonl
```

Search used the same frozen baseline library with `search --policies
blue-eyes-improved --opponents blue-eyes --rules mr5 --games 32 --seed 4100000
--workers 8 --limit 8192 --worlds 8 --confirm 32 --final 96 --log true
--record true --library "$scratch/baseline.so" --output blue-eyes-search.jsonl`.

## Records and checkout

The checkout was fetched from `origin`. Its benchmark branch already contained
the fetched upstream; remote `main`'s extra search-logging commit was merged as
`6f2c7c4`, retaining the benchmark branch's outside-player search support.
Work lives on `codex/blue-eyes-improvement`.

Every run has compressed raw JSONL, adjacent metadata, and a summary. Search
records include observations and replay responses. Rows retain the actual engine seeds; metadata records the rules, library
fingerprints, deck/database fingerprints and pinned engine/Lua/script revisions. `manifest.json` adds SHA-256 hashes and the compiler
version; `validation.json` verifies the seed split and records seat-specific counts.
`variants/*.patch` reconstructs each tested version from a fresh copy of the
original source, including the rejected Veiler variant. `variants.json` identifies
the development source and library hashes. Temporary shared libraries are not
committed; rebuild them from the sources and patches. `workspace-tests.log`
contains the successful complete test run.
