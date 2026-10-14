# Fresh confirmation: Chain Burn manual weights

The frozen manual preset won **77/256 fresh mirrors (30.08%)**, compared with
**59/256 (23.05%)** for the defaults. It improved 28 identical seed/seat games,
regressed 10, and retained the outcome of 218. The net gain is **7.03125 percentage
points**. This is a clear positive signal for this preset on this matchup, while
its absolute win rate remains well below the 55% target.

| Fresh paired comparison | Result |
| --- | ---: |
| Default wins | 59/256 |
| Manual wins | 77/256 |
| Gain | +7.03 percentage points |
| Approximate paired-normal 95% interval | +2.38 to +11.68 points |
| Supplementary paired Newcombe 95% interval | +2.32 to +11.76 points |
| Exact two-sided McNemar p-value | 0.00509764 |
| Manual win/default win | 49 |
| Manual win/default loss | 28 |
| Manual loss/default win | 10 |
| Both lose | 169 |

There were no draws. These results use **only the 256 fresh pairs**; the original
64-pair pilot is excluded. Both arms had higher win rates on the new seeds, so
30.08% must be compared with this sample's 23.05% default control, not the pilot's
14.06%. The earlier pilot gain was +6.25 points and uncertain. The independent
confirmation has the same direction and a similar magnitude.

## Preset and controlled scope

The preset remains exactly `accessible_cards=0.3` and `life_buffer=4.0`, compared
with defaults of 0.8 and 0.55. The other 11 coefficients are unchanged. It was
chosen before the pilot and was not retuned for confirmation. No policy features,
card facts, search parameters or runtime deck profiles were added. Default policy
settings remain unchanged.

The result supports the premise that playstyle-appropriate valuation can improve
search on this deck. It does not identify which of the two coefficients caused
the gain, show that these are optimal weights, validate automatic deck-profile
inference, or establish improvement on other or unknown decks. The practical
five-point threshold in the plan is a relevance threshold; the interval does not
establish a gain of at least five points.

Both arms used the same frozen executable, policy library and specialist baseline,
with the same deck, rules, seed/seat schedule and search budget. The analyzer
checks that all fixed inputs also match the pilot, except the deliberately fresh
schedule. The only two configuration differences are the named coefficients.
The full plan was saved before launch, and both arms finished their fixed samples
before any outcome analysis. There was no early stopping or additional preset
selection.

- 256 games per arm, 128 per candidate seat; schedule seed 2475000.
- Engine seeds 1196573947 through 1196574202, disjoint from the pilot.
- Eight workers per arm, 16 total, pinned to CPUs 8–23.
- Same MCTS budget: 128 simulations, four proposal worlds, eight actions, 1024
  nodes, 384 decisions, one reply, four fresh validation worlds, best-root
  validation, turn-horizon evaluation, and no wall-clock cutoff.
- Generic simulated opponent and adversarial opponent UCT disabled. Search still
  assumes known opposing deck composition.
- Frozen executable source `5705ae27e15fa7b223f2eeb1833232fb4eb6265f`; policy library
  source `875bea849cb58ca4d5c31f1e322b2ea48355441d`. The launch checkout was
  `da32abb1c4662248e3d1d9c8273bf120f1174f0a`, which added pilot evidence only.

## Completion and performance

All 512 games completed. Both arms had zero duel errors/caps, failed searches,
failed simulations, proposal cutoffs, unresolved validation, exhausted work
budgets and unknown-facedown search skips. All 2,031,360 default and 1,930,624
manual proposal simulations completed. Short-horizon validation intentionally
used heuristic leaves in 83,779/126,960 default continuations and 82,419/120,664
manual continuations; these ordinary truncations are not failures.

| Searched decisions only | Default | Manual |
| --- | ---: | ---: |
| Count | 15,870 | 15,083 |
| Median | 577 ms | 583 ms |
| Mean | 622 ms | 636 ms |
| 95th percentile | 1092 ms | 1146 ms |
| Arm wall time | 1294 s | 1225 s |

The arms ran concurrently and the full benchmark took about 21.6 minutes. Timing
under 16-worker load is descriptive: changed trajectories mean this is not a
controlled speed comparison. This run does not demonstrate the desired 200 ms
decision latency. It is a strength confirmation with the original work budget.

## Audit and reproduction

[Plan](plan.json), [manifest](manifest.json), and
[artifact verification](artifact-verification.json) record exact commands,
coefficients, source revisions and binary SHA-256 fingerprints, verified before
and after the run. Both benchmark processes exited successfully. Raw evidence is
stored losslessly in `default.jsonl.gz` and `manual.jsonl.gz`; the parent archive
manifest records SHA-256 hashes of their decompressed bytes.

[Comparison](comparison.json) and [paired statistics](paired-statistics.json)
contain exact configuration differences, full telemetry and input fingerprints.
The confirmation analyzer rejects partial schedules, pilot-seed overlap, changed
fixed inputs, unmatched binaries and unplanned settings. Its supplementary
[paired Newcombe interval](https://www.statsmodels.org/dev/generated/statsmodels.stats.proportion.confint_proportions_paired.html)
was checked against the documented example and sign reversal; the original
paired-normal interval and exact McNemar test are also reported. The score-based
interval and exact test corroborate the positive direction.

No Rust implementation changed or was rebuilt for this confirmation. The previously
tested frozen build was reused byte-for-byte. Metadata checks, formula checks and
independent read-only analysis review passed.

To rerun, use the manifest commands with fresh output paths and the pinned
artifacts. Run `python3 -B analyze_confirmation.py` only after both arms complete;
it refuses to overwrite derived reports. Move existing `comparison.json` and
`paired-statistics.json` aside before reproducing analysis, then compare the new
outputs. Raw inputs are never modified by analysis.
