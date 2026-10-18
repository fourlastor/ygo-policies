# Direct chain-payoff diagnostic

The new library **did not demonstrate a strength benefit** on the fixed Chain Burn sample.

| Mode | Control wins | Candidate wins | Gained / lost wins | Paired change, normal 95% interval |
|---|---:|---:|---:|---:|
| Plain Generic | 39/128 | 38/128 | 0 / 1 | −0.78 points, −2.31 to +0.75 |
| MCTS, 32 iterations | 24/128 | 24/128 | 4 / 4 | 0.00 points, −4.35 to +4.35 |

There were no draws. Trajectory digests changed in 30 plain pairs and 99 search pairs; changed play did not produce a positive aggregate result. Equal search win counts do not establish equivalence. Candidate rates remain far below 55%; this one-deck development diagnostic does not qualify the broader target.

All four arms completed exactly 128 unique games, 64 per candidate seat, at fresh base seed **2492000**. Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion, proposal cutoffs, soft budget overruns and plan/replay mismatches were zero. Independent checks reproduced both saved comparisons, exact schedules, metadata arguments, default weights, validation records and frozen fingerprints.

The policy change permits an already offered, useful **Chain Strike or Accumulated Fortune** response above the policy's own top chain link, with known-chain threshold and duplicate-link guards. Fortune also reserves deck capacity for known pending draws. This models direct offered payoffs; it does **not** plan future chain construction, grant activation permission to other draw cards or fully model runtime name changes. The only changed policy source file was `generic/life.rs`.

Within each mode, only the library changed: v21 **aa96d86cf036b7c1af9902e43e8642329d97a0b3** to v24 **bc4beeab208ccecbfa4a9ed915606e0f794d21a4**. Both used the same frozen v24 host, unchanged default weights, and explicit `--mcts-root-warmup false`. The host's restored UCT default and face-down banished-card guard were shared by both arms and are not isolated by this comparison. Known opponent composition remained permitted; strict search retained hidden-information guards.

The raw `search_skipped_facedown_banished` counter was **zero in all four arms**, including 20,591/20,023 candidate decisions in the searched control/candidate arms. These counts do not demonstrate the new guard being exercised. Fresh searches numbered 8,600/8,150; active-chain skips 1,123/1,043, unknown-monster and unsafe-selection skips zero, and cached continuation hits 567/578. Guard counts concern eligible fresh-search prompts after earlier checks, not all decisions with those conditions. All 275,200/260,800 proposal simulations completed; 46,979/44,621 heuristic validation playouts ended at their configured horizon rather than terminal outcomes.

[validation.json](validation.json) records 21 life-policy tests, 87 engine unit tests, five native MCTS checks and 25 analyzer tests passing, plus 86 historical identities preserved. The banished-card guard has unit/wire coverage; no native Doom Lord/Slip duel fixture was run. This audit reran no tests. At 16 workers, searched-decision medians were 141/143 ms and p95 values 245/256 ms; these loaded, trajectory-dependent measurements are not a latency guarantee.

[plan.json](plan.json), [manifest.json](manifest.json) and [independent-analysis.json](independent-analysis.json) preserve commands, builds, hashes and raw-counter checks. All four raw outputs are archived with deterministic gzip. Archive records verify logical/compressed hashes, exact decompression and unchanged comparison metrics and identities except input paths. Original pinned files remain unchanged; `.jsonl` references resolve to adjacent `.gz` files.
