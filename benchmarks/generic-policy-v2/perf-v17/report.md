# Answer-reuse performance diagnostic

The new host reduced searched-decision latency on this four-game Demise check while preserving the recorded outcomes, trajectory digests and search-work counters. **Not all non-timing fields matched:** the first game's snapshot byte-volume counters differed as detailed below.

| Searched-decision latency | Previous host | Answer-reuse host |
|---|---:|---:|
| Median | 189.52 ms | **156.50 ms** |
| Mean | 197.43 ms | **162.62 ms** |
| p95 | 314.40 ms | **249.98 ms** |
| Maximum | 407.33 ms | **318.91 ms** |

All 102 corresponding searched decisions were faster, paired by game and recorded decision order; the median paired saving was 32.31 ms. The aggregate median fell 17.4%. These are four fixed games from one deck, measured in separate serial single-worker runs without concurrent benchmarks or builds. They are descriptive performance evidence, not independent strength evidence or a general speed guarantee. The p95 still exceeds 200 ms, and no wall-time cap was enabled.

Both runs won 1/4, with identical winners and digests for every game. All 3,264 proposal simulations completed; search counts, retained/visited actions, acceptance, decisions, turns, snapshot counts and restore counts matched. Errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion and proposal cutoffs were zero.

For seed **110481259**, candidate seat 0, all **17 searched roots** had `max_snapshot_bytes` higher by **1,536 bytes**, `snapshotted_bytes` higher by **16,896 bytes**, and `restored_bytes` higher by **78,336 bytes** per root. Each root still recorded 11 snapshots and 51 restores. The other 85 roots matched exactly, including copy volumes. Aggregate differences were +287,232 snapshotted bytes and +1,331,712 restored bytes. These counters measure logical monotonic-arena allocation high-water usage, including prior allocations, rather than live state size. The exact cause of the offset is unproven; this is not a claim of byte-identical state. [parity-details.json](parity-details.json) records every difference.

The schedule was seed **2483000**, four games, one worker on CPUs 8–23. Only the host executable changed: source **5705ae27e15fa7b223f2eeb1833232fb4eb6265f** → **62614aedd48a4cbaa17695b179d9bc68592160fb**. The v16 candidate library, specialist baseline, fixed inputs, default weights and 32-iteration MCTS settings stayed unchanged. [manifest.json](manifest.json) embeds the new executable's exact build record, artifact hashes and command; the [reference plan](../fast32-v16/plan.json) preserves the old host and unchanged library provenance. The shared analyzer reproduced [comparison.json](comparison.json) with only the executable-revision allowance.

The new raw file is archived as `reuse.jsonl.gz`; the reference archive remains unchanged. [archive-manifest.json](archive-manifest.json) records logical/compressed SHA-256 hashes. [archive-verification.json](archive-verification.json) confirms round-trip integrity and identical comparison metrics and identities after compression, excluding input filenames. Original manifests, metadata, summary and comparison are unchanged; their `reuse.jsonl` references resolve to the adjacent `.gz` file.
