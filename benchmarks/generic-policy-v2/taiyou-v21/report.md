# Taiyou target diagnostic

The target prior produced positive but **uncertain** point estimates on the fixed Deckout sample; a strength improvement is not established.

| Mode | Control wins | Candidate wins | Gained / lost wins | Paired change, normal 95% interval |
|---|---:|---:|---:|---:|
| Plain Generic | 9/64 | 11/64 | 3 / 1 | +3.13 points, −3.00 to +9.25 |
| MCTS, 32 iterations | 6/64 | 10/64 | 5 / 1 | +6.25 points, −1.15 to +13.65 |

Both search arms drew twice; draws count as non-wins. Plain/search trajectory digests changed in 19/42 pairs. Candidate rates remain far below the 55% target. This is one-deck development evidence, not broader qualification.

All four arms completed exactly 64 unique games, 32 per candidate seat, at fresh base seed **2489000**. Independent checks reproduced both comparisons and validated schedules, summaries, metadata, quality and frozen fingerprints. Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion and proposal cutoffs were zero. All 28,736 control and 26,464 candidate proposal simulations completed.

Search coverage uses **candidate-seat decision counts**, not both players' decisions:

| Recorded event | Control: 8,271 candidate decisions | Candidate: 8,320 candidate decisions |
|---|---:|---:|
| Fresh searches | 898 (10.9%) | 827 (9.9%) |
| Fresh search skipped: unknown opposing face-down monster | 2,430 (29.4%) | 2,660 (32.0%) |
| Fresh search skipped: active chain | 788 (9.5%) | 781 (9.4%) |
| Cached MCTS continuation applied | 197 (2.4%) | 167 (2.0%) |

The skip counters apply only after a noncached, supported prompt offers multiple distinct alternatives. Among prompts reaching those guards (**4,116 control; 4,268 candidate**), unknown-monster skips account for 59.0%/62.3%, chain skips 19.1%/18.3%, and fresh searches 21.8%/19.4%. Guards check chain first, then unknown monsters, then unsafe selection (zero here), so these are exclusive first-reason counts, not all prompts exhibiting each condition. Cached continuations are checked before those guards.

Cache hit rates were **197/3,088 (6.4%)** and **167/3,214 (5.2%)** among decisions where a cached tree existed. Cache misses can subsequently enter a fresh-search guard and must not be added as another disjoint category. [search-coverage.json](search-coverage.json) records raw totals, denominators and frozen-source definitions. These counts locate current fresh-search restrictions; they do **not** establish that skipped opportunities are safely recoverable or would improve wins.

The fixed capacity-v18 host was **aea6dd818208509eb382edc69caa31893749c42e**. Within each mode, only the library changed from **fb1178356392f38f535a4d15bce97ab3839854ae** to **aa96d86cf036b7c1af9902e43e8642329d97a0b3**. The Taiyou prior favors an offered own Needle Worm with the recorded guards; activation behavior is unchanged, and flipping it is not guaranteed to be beneficial. Default weights, specialist baseline and search settings stayed fixed. Known opponent composition was permitted; hidden arrangements remained sampled under strict search.

[validation.json](validation.json) records **45 transaction tests passing**, including whole-card and Toggle selection fixtures. This audit reran no tests. Arms ran serially at 16 workers on CPUs 8–23; loaded candidate search latency (169 ms median, 278 ms p95) is not a latency guarantee.

[plan.json](plan.json), [manifest.json](manifest.json) and [independent-analysis.json](independent-analysis.json) preserve commands, builds, hashes and audit results. All four raw files are archived as deterministic `.jsonl.gz` files. Archive records verify logical/compressed hashes, exact decompression and unchanged comparison metrics and identities except input paths. Original plans, runner, metadata, validation and comparisons remain unchanged; `.jsonl` references resolve to adjacent `.gz` files.
