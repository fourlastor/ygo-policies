# Psychic Commander transaction diagnostic

The bundled change produced an **uncertain two-win plain gain and no aggregate search gain**. It does not establish stronger search play or qualify the 55% target.

| Mode | Control wins | Candidate wins | Gained / lost wins | Paired change, normal 95% interval |
|---|---:|---:|---:|---:|
| Plain Generic | 55/128 | 57/128 | 2 / 0 | +1.56 points, −0.59 to +3.72 |
| MCTS, 32 iterations | 63/128 | 63/128 | 7 / 7 | 0.00 points, −5.75 to +5.75 |

There were no draws. Trajectory digests changed in 36 plain pairs and 71 search pairs. Equal search win counts do not establish equivalence. Candidate observed rates were 44.5% plain and 49.2% searched; both pointwise Wilson lower bounds remain below 55%. These are fresh one-deck development samples, not broader qualification.

The fixed schedule used base seed **2493000**, 128 unique games per arm and 64 per candidate seat, with four serial arms. Independent checks reproduced both saved comparisons and verified complete schedules, metadata, default weights, frozen fingerprints and validation records. Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion, proposal cutoffs, soft budget overruns and plan/replay mismatches were zero. All 97,376 control and 91,712 candidate proposal simulations completed; 18,584/17,553 heuristic validation playouts ended at their configured horizon rather than a terminal outcome.

This is a **combined FFI adapter and Commander transaction change**, not an isolated payment-choice experiment. Each loaded FFI uses its own `ocgcore::Seat` adapter. The v24 control retains the old numerical hint behavior; the v25 candidate preserves the AnnounceNumber hint and adds guarded Commander activation/payment logic. The runtime source diff also includes Generic announcement routing and shared combat-helper extraction. No Exodia or evaluator-weight source changed. Multi-link transactions and unmodelled damage shields retain fallback behavior.

Only the loaded library differed within each mode: **bc4beeab208ccecbfa4a9ed915606e0f794d21a4** versus **7deeff112a190c2f93e017805d9dd4a4fc72eebc**. All arms used the same frozen v25 host, unchanged default weights, UCT and explicit `--mcts-root-warmup false`. The motivating [historical Commander trace](../weak-diagnostic-v17/diagnosis.md) identified a payment mechanism; it did not establish an alternative match outcome.

Fresh searches numbered 3,043/2,866; active-chain skips 950/906, unknown-monster skips 1,178/1,140, and cached continuation hits 809/868. The face-down banished-card skip counter was present and zero in all four arms, so this sample does not demonstrate that guard being exercised. Full raw-counter totals are in [independent-analysis.json](independent-analysis.json).

[validation.json](validation.json) records **316 policy tests**, including the 15 focused combat tests, and **33 adapter tests** passing. This audit reran no tests. Builds overlapped the benchmark on the pinned CPUs during part of the run. The 16-worker timings are therefore loaded and incidental; they support no controlled performance claim.

[plan.json](plan.json) and [manifest.json](manifest.json) preserve commands, source/build records and hashes. All four raw files are archived with deterministic gzip. Archive records verify logical/compressed hashes, exact decompression and identical comparison metrics and identities except input paths. Original pinned files remain unchanged; `.jsonl` references resolve to adjacent `.gz` files.
