# Legal-menu host diagnostic

The new host produced an **uncertain three-win aggregate gain**, from **96/192 (50.0%) to 99/192 (51.6%)**. It gained 11 paired wins and lost eight, with 173 outcomes unchanged. The paired change was **+1.56 percentage points**, normal 95% interval **−2.89 to +6.02 points**. This does not establish an overall strength improvement.

| Deck | Control wins | Candidate wins | Gained / lost wins | Changed trajectory digests |
|---|---:|---:|---:|---:|
| Crystal | 36/64 | 43/64 | 7 / 0 | 23 |
| Chain Burn | 16/64 | 16/64 | 0 / 0 | 0 |
| Fabled | 44/64 | 40/64 | 4 / 8 | 34 |

Effects varied by deck. Identical Chain Burn outcomes and digests in this sample do not prove general equivalence. No candidate deck's pointwise Wilson 95% lower bound cleared 55%: Crystal's is **54.9989%**, just below the threshold despite rounding to 55.0%. These three selected decks and 64 games per deck do not qualify the full-roster goal. Full per-deck intervals and counts are preserved in [independent-analysis.json](independent-analysis.json).

The fixed sample used fresh base seed **2488000**, 192 paired games per arm, 64 per deck and 32 per candidate seat. Both arms completed all scheduled games with zero errors, live caps, failed searches/playouts, unresolved validation, incomplete worlds, budget exhaustion or proposal cutoffs. All 280,704 control and 276,096 candidate proposal simulations completed. The configured heuristic horizon produced 52,755 and 51,378 truncated validation playouts; these were bounded evaluations rather than failures or terminal proofs. Independent checks reproduced the saved comparison, quality totals, schedules, metadata arguments and frozen fingerprints.

Only the host changed, from **aea6dd818208509eb382edc69caa31893749c42e** to **47111be9977263668ec323ff7ae85c936c287751**, which qualifies immediate search value using engine-provided legal actions. The v19 library (**fb1178356392f38f535a4d15bce97ab3839854ae**), default weights and MCTS32 settings stayed fixed. Known opponent deck composition is intentional; hidden arrangements remain sampled under strict search, using the Generic opponent model.

[validation.json](validation.json) records **79 unit tests and all 10 native integration tests passing** for the frozen candidate source and matching v19 library before the strength run. Native coverage includes legal-menu access, both seats, reply horizons, terminal bypass, replay and restoration. This artifact audit did not rerun tests.

Arms ran serially with 16 workers on CPUs 8–23. Loaded median search latency was 214 ms control and 188 ms candidate; p95 was 611 ms and 544 ms. Different trajectories, search counts and worker contention prevent a causal latency claim or a general speed guarantee.

[plan.json](plan.json) and [manifest.json](manifest.json) preserve exact commands, effective settings, source/build records and artifact hashes. Both raw outputs are archived as deterministic `.jsonl.gz` files. Original metadata, analyses, validation and pinned runner remain unchanged; `.jsonl` references resolve to adjacent `.gz` files. Archive records confirm logical/compressed hashes, byte-for-byte round trips and identical compressed-input comparison metrics and identities, excluding input paths.
