# One 32-iteration MCTS diagnostic

Reducing MCTS iterations from 128 to 32 produced **35/64 wins versus 32/64** on the same Demise schedule. The paired change was **+4.7 percentage points**, with 12 gained wins, 9 lost wins and a paired normal 95% interval of **−9.4 to +18.8 points**. This small development sample does not establish either stronger play or preserved strength. The observed 54.7% also falls below the 55% target; its Wilson 95% interval is 42.6–66.3%.

This was one fixed variant, chosen after reviewing the 128-iteration results, using the existing 64-game schedule at base seed **2482000** (32 games per seat). It is a same-seed development ablation, not fresh confirmation. Both runs used five workers. The comparison validated exactly one configuration difference, `search.mcts.iterations: 128 → 32`, with unchanged binaries, default evaluation weights and fixed inputs.

A separately authorized single-worker latency repeat used the same four Demise games at seed **2483000**, with no concurrent benchmark or build:

| Iterations | Searched decisions | Median | Mean | p95 | Maximum |
|---|---:|---:|---:|---:|---:|
| 128 | 111 | 900.31 ms | 929.96 ms | 1,483.42 ms | 1,902.60 ms |
| 32 | 102 | **189.52 ms** | **197.43 ms** | **314.40 ms** | **407.33 ms** |

Both latency samples won 1/4. The roughly 4.75× lower median is descriptive: resulting trajectories and search-decision counts differ, so these are not paired decision timings. The 32-iteration median and mean are below 200 ms in this sample, but p95 and maximum are above it. No wall-time cap was enabled, and this is not a general latency guarantee.

Both new runs completed without game errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion or MCTS proposal cutoffs. The 64-game run completed all 48,928 proposal simulations. Heuristic-horizon validation still recorded 9,236 truncated playouts (516 in the latency repeat); these are bounded heuristic evaluations, not terminal proofs or failed games.

The frozen executable source is **5705ae27e15fa7b223f2eeb1833232fb4eb6265f**, and the unchanged v16 library source is **9bacb1307f988af737c788ea844cfdc4cd902ac5**. [plan.json](plan.json) contains their exact SHA-256 hashes, the embedded library build record, the reference fingerprints and full command; [manifest.json](manifest.json) preserves launch provenance. [latency-manifest.json](latency-manifest.json) records the second command. The strength comparison requires the explicit shared-analyzer declaration `--allow-metadata-ablation search.mcts.iterations`, without a candidate-revision override. Results are in [comparison.json](comparison.json) and [latency-analysis.json](latency-analysis.json).

Completed raw data in this folder and the [reference folder](../material-search-v16/report.md) are archived as deterministic `.jsonl.gz` files. Original manifests, metadata, analyses and their `.jsonl` path strings remain unchanged; use the adjacent `.gz` file when resolving an archived input. Archive manifests record logical and compressed SHA-256 hashes, and archive verification records confirm round-trip integrity and unchanged comparison metrics and identities.
