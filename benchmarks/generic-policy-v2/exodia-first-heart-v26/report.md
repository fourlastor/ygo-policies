# First-Heart selection diagnostic

The candidate produced **unchanged plain wins and an uncertain one-win search gain**. This does not establish stronger play or qualify the 55% target.

| Mode | Control wins | Candidate wins | Gained / lost wins | Paired change, normal 95% interval |
|---|---:|---:|---:|---:|
| Plain Generic | 31/128 | 31/128 | 0 / 0 | 0.00 points; degenerate interval |
| MCTS, 32 iterations | 31/128 | 32/128 | 1 / 0 | +0.78 points, −0.75 to +2.31 |

The plain sample has no discordant win outcomes, so its mechanically calculated normal interval is [0, 0]. That is **not evidence of equivalence**. Trajectory digests changed in four plain pairs and nine search pairs. There were no draws. Candidate observed rates were 24.2% plain and 25.0% searched; both Wilson lower bounds remain below 55%.

The fixed fresh Exodia schedule used base seed **2494000**, 128 games per arm and 64 per candidate seat, with four serial 16-worker arms on CPUs 8–23. Independent checks reproduced the saved comparisons exactly and verified all 512 records, schedules, metadata, default weights and frozen artifact hashes. Only the loaded library changed within each mode: v25 **7deeff112a190c2f93e017805d9dd4a4fc72eebc** versus v26 **0a3c26cd1b04c05a9b9403277d618f69103e7fc4**. The frozen v25 host, generic simulated opponent, UCT default and explicit `--mcts-root-warmup false` were identical. Timing was collected with 16 workers and is descriptive, not a controlled latency comparison.

The source diff changes only `generic/alternate.rs`. During an exact own Pot of Duality single-card selection, it prefers the first supported Heart of the Underdog, preserves completion of the fifth Exodia piece, requires a known chain context, and conservatively checks installation space while Duality resolves. It does not change evaluator weights or general card retention. The v25 Announce adapter and Commander logic are common prerequisites. The [selected historical trace](../weak-diagnostic-v17/diagnosis.md) motivated the selection rule; it did not prove that another choice would change the match result. Crowded additional Hearts and Reload choices are separate unresolved mechanisms.

Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion, proposal cutoffs, soft budget overruns and plan/replay mismatches were all zero. All 11,840/11,712 control/candidate proposal simulations completed. Heuristic validation reached its configured horizon in 2,957/2,925 playouts; these are not live-game caps.

Search coverage remained small: 370/366 fresh searches among 26,041/26,086 candidate decisions (about 1.4%). Unknown-monster skips were 17,161/17,189, active-chain skips 3,130/3,224, and cached continuation hits 101/102. Face-down banished-card skips were recorded and zero. These are observed coverage limits, not a promise that skipped prompts can be safely recovered. Full counters appear in [independent-analysis.json](independent-analysis.json).

[validation.json](validation.json) records **13 alternate-rule tests and 322 policy tests** passing. This audit reran no tests. [plan.json](plan.json) and [manifest.json](manifest.json) preserve exact commands, build records and artifact hashes. The exact analyzer is saved as [analyzer.py](analyzer.py). All four raw files are archived with deterministic gzip; archive records verify logical and compressed hashes, exact roundtrips and unchanged comparison metrics and identities except input paths. Original pinned evidence remains unchanged, with `.jsonl` references resolving to adjacent `.gz` files.
