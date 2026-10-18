# Root-world warmup diagnostic

Root warmup **did not demonstrate a strength improvement**. Wins changed from **71/128 (55.5%) to 66/128 (51.6%)**, with 10 paired gains and 15 losses. The paired difference was **−3.91 percentage points**, normal 95% interval **−11.56 to +3.75 points**. This diagnostic does not establish an aggregate regression either. Warmup is retained as experimental rather than adopted as the default.

| Deck | Control wins | Warmup wins | Gained / lost wins | Changed trajectory digests |
|---|---:|---:|---:|---:|
| Crystal | 18/32 | 18/32 | 5 / 5 | 18 |
| Gem-Knight | 17/32 | 19/32 | 3 / 1 | 17 |
| Gishki | 16/32 | 15/32 | 1 / 2 | 15 |
| Heroes | 20/32 | 14/32 | 1 / 7 | 18 |

The fixed sample used fresh base seed **2490000**, 32 games per deck and 16 per candidate seat. These selected four decks and small per-deck samples do not qualify the full-roster goal. Two candidate decks had observed rates above 55%; none had a pointwise Wilson 95% lower bound above 55%. Per-deck paired intervals are exploratory and not adjusted for multiple comparisons.

All **16,186 retained candidate root actions** were both attempted and successfully evaluated in every supplied proposal world. These counts aggregate root actions across 3,803 fresh searches, not unique game actions; validation worlds are excluded. The old host did not record those counters, so **control all-world coverage is unknown**. Both hosts visiting each retained action at least once does not establish that each saw every world. Full candidate coverage alone is not evidence of better choices or stronger play. [root-coverage.json](root-coverage.json) preserves the definitions and totals.

Both arms completed all 128 games. Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion and proposal cutoffs were zero. All 120,192 control and 121,696 candidate proposal simulations completed. The heuristic horizon produced 24,186 and 24,304 truncated validation evaluations; these are bounded evaluations, not terminal proofs.

Only the frozen host changed: **47111be9977263668ec323ff7ae85c936c287751** to **2b8d7862a4bbf2b2b94455ac2a04a5508b348d7e**. Both used the same v21 library, **aa96d86cf036b7c1af9902e43e8642329d97a0b3**, unchanged default weights, 32 iterations, four proposal worlds and four fresh validation worlds. The intervening Taiyou source change in the host is not used by the loaded Generic pilots. Metadata catalogs matched exactly. Known opponent composition remained permitted; hidden arrangements were sampled under strict search.

The frozen plan records **16 focused unit tests and five native MCTS tests passing** before launch, with the matching v21 library. This audit reran no tests. Loaded search medians were 235/242 ms and p95 values 513/529 ms at 16 workers. Different trajectories and worker contention prevent an isolated overhead claim or latency guarantee. The separately prepared `perf-v22` single-worker plan was not launched after the decision to keep warmup experimental.

[plan.json](plan.json), [manifest.json](manifest.json) and [independent-analysis.json](independent-analysis.json) preserve commands, source/build records, hashes and independent checks. Both raw outputs are archived with deterministic gzip; archive records verify logical/compressed hashes, exact decompression and identical shared comparison metrics and identities except input paths. Original pinned files remain unchanged; `.jsonl` references resolve to adjacent `.gz` files.
