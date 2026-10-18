# Frozen v9 performance comparison

All **eight game pairs** matched exactly on their complete non-timing rows,
including outcomes, response digests, decisions and all recorded search counters.
Each version executed 290 searches, 37,120 completed proposal simulations and
2,320 validation trials. There were no game errors, live caps, failed searches,
failed playouts, unresolved validation trials or proposal cutoffs. These checks
establish parity for this fixed sample, not universal behavioral equivalence.

The experiment compares a bundle of host snapshot/field-query allocation changes
and FFI message/query parsing changes. Both executable and library changed.
Generic-policy and evaluator sources were unchanged between the two committed
revisions; later card-rule changes are excluded from these frozen binaries.

| Version | Source revision | File | SHA-256 |
| --- | --- | --- | --- |
| Control | `1576a5905802f8e2e2c696abc7796ec0d0bb7ae3` | executable | `ed4c6f8a119d03a3650f4db02705fc1dda3065a49c184de7e1f6a84aaa2e65d7` |
| Control | same | FFI library | `9c3e60325a835b2025191cdf8cc5f86bb2b42f8650c980415ca5c1cce0d307d1` |
| Optimized | `2b289c6f3f7e545e5335e39ee5d396313da63a2d` | executable | `d1280e127eb55a4372f75a585f4a3339bb22b0f59c9d946d7e7a7e0177079eec` |
| Optimized | same | FFI library | `3df62caf14688b50bc8c69c8d82c253310bb32cb6b345f30db0ca0d4f67303e0` |

The [manifest](manifest.json) records exact commands, binary sizes and runner
fingerprint. The frozen baseline, card database, scripts, engine, deck files,
rules and paired seeds match. Each version used its own library and the same
small MCTS configuration: 128 iterations, four proposal worlds, four validation
worlds, eight root candidates/actions, 1024 nodes, 384 decisions, one reply,
best-only validation, heuristic horizon 1, 256 total proposal/validation attempts
and no time deadline. Two games per deck used seed 2468000 and alternating seats.

Measurements ran one worker on CPUs 8–11, alternating version order across
Blackwing, Fabled, Monarch and Worm. The v8 background benchmark was confined to
CPUs 12–23. It remained active throughout Blackwing control and exited during
Blackwing optimized, so those two pairs have asymmetric shared-load exposure.
The [event log](events.jsonl) records this transition, successful exits and CPU
masks. Cleanup found that v8 had already exited; no surviving process required
affinity restoration. The prior failed attempt is preserved in the separate
`../perf-v9-2b289c6` directory and launched no measurements.

The six later game pairs—Fabled, Monarch and Worm—had no competing benchmark
process during either version's run:

| Metric | Control | Optimized |
| --- | ---: | ---: |
| Total search time, 234 matched searched decisions | 223.255 s | 211.885 s |
| Search median | 891 ms | 831 ms |
| Search p95 | 1685 ms | 1583 ms |
| Total candidate decision time, 1804 matched decisions | 223.611 s | 212.239 s |

Total search time was **5.09% lower** in these six pairs. The mean of the six
per-game reductions was 4.88%, with individual reductions from 3.51% to 5.95%.
Every measured game's total search time was lower, including the two excluded
Blackwing pairs. Across all eight pairs, search time was 284.435 to 269.759
seconds (5.16% lower), but that aggregate includes the background-load change.

These are descriptive observations from one pass over six primary timing pairs,
not a global speed guarantee or a confidence interval. Decisions within a game
are correlated; 234 decisions are not 234 independent experimental units.
Unrelated system activity and temporal drift were not controlled. Search still
takes roughly a second per searched choice in this sample. No conclusion about
the 55% per-specialist strength target follows from this performance experiment.

The original [paired report](comparison.json) and the derived
[background-separated report](latency-groups.json) preserve exact values and
input hashes. Raw `.jsonl`, metadata and summary files remain uncompressed because
the event log binds their hashes. To reproduce the derived report without
launching games or modifying inputs, run from the repository root:

```sh
/usr/bin/python3 benchmarks/generic-policy-v2/summarize_perf_v9.py \
  benchmarks/generic-policy-v2/perf-v9-2b289c6-r2
```

The helper first reconstructs and verifies the original comparison in memory,
then separates groups using recorded background presence at each block's start
and end. It emits JSON to standard output.
