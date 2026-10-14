The Demise sequencing fix improved the fresh paired Demise samples while preserving every recorded outcome and digest on the other 70 decks in the plain screen. The fresh MCTS result is **38/64 wins (59.4% observed)**. Its Wilson 95% interval is **47.1–70.5%**, so this does not establish a true win rate of at least 55%, or achievement of the per-deck roster goal.

The intervention adds Demise to the existing paid-wipe logic: account for its 2,000 LP payment and friendly collateral, reject spending the last LP, and use a profitable wipe before developing additional cards onto the field. Only the loaded candidate policy library changed. Both arms used the same frozen executable, specialist baseline, card/script/deck inputs, all 13 default evaluation weights, and search settings. There was no weight tuning or new search feature in this comparison.

| Sample | Matched pairs | Control wins | Candidate wins | Paired gain, approximate normal 95% interval |
|---|---:|---:|---:|---:|
| Plain, all 71 decks | 4,544 | 1,640/4,544 | 1,650/4,544 | +0.22 pp [0.07, 0.37] |
| Demise subset of that plain screen | 64 | 17/64 | 27/64 | +15.63 pp [5.65, 25.60] |
| Fresh Demise MCTS pilot | 64 | 23/64 | 38/64 | +23.44 pp [9.81, 37.06] |

The plain Demise pairs contain 11 improvements, one regression, and 52 unchanged win outcomes. All 4,480 pairs from the other 70 decks, including Countdown, have identical winners and digests across arms. Excluding Countdown, the required 70-deck aggregate is 1,620→1,630 wins in 4,480 pairs. The MCTS pairs contain 19 improvements, four regressions, and 41 unchanged win outcomes. Draws count as non-wins. Intervals are paired win differences; the Demise plain row is a subset, not an additional sample.

The original eight same-seed diagnostic games (base seed 2476000) motivated the fix and were replayed for behavioral checks. The recorded v13 traces contained seven own-only wipes and three activations spending the last 2,000 LP; these were absent in the v14 replay. Wins changed from 1/8 to 4/8 on those selected diagnostic games. They are **excluded from the fresh strength results**. [Diagnostic observations](diagnosis-observations.json) retain the exact seeds and trace indices.

The plain stage used fresh base seed **2477000**, 64 games per deck and 32 candidate games in each seat. Only after its predeclared gate passed did the separate Demise stage use base seed **2478000**, again 64 games and balanced seats. Both fixed samples completed. Full raw-record recomputation matched the saved analyses, and both gates passed. Across both stages and both arms there were zero invalid games, live caps, failed searches/playouts, unresolved validation, incomplete worlds, exhausted search budgets, or MCTS proposal cutoffs.

MCTS retained the default 128 iterations, four proposal/validation worlds, eight actions/candidates, 1,024 nodes, 384 decisions, one reply, `max_playouts=256`, heuristic validation, best-only root validation, generic opponent model, and no time cap. Control/candidate completed 132,480/182,656 proposal simulations over 1,035/1,427 searched decisions. Heuristic truncation was intentional and is distinct from a failed search or proposal cutoff.

Search-decision median/p95 latency was 603/1,091 ms for control and 645/1,162 ms for candidate. These are descriptive measurements under 16 concurrent benchmark workers on CPUs 8–23; a concurrent pinned two-job build ran during part of MCTS. The arms also encountered different numbers of decisions. This is not a controlled speed comparison.

Frozen artifact provenance is recorded below. The live launch checkout was `2757223caec30f06d677dbda49c684f69fc45490`; its independent edits do not identify the source of the frozen binaries.

| Artifact | Source commit | SHA256 |
|---|---|---|
| Shared v13 executable | `5705ae27e15fa7b223f2eeb1833232fb4eb6265f` | `2c2fdadb64a861743561d703da3a684e4783eadfa041097532ced50458f9df66` |
| Control library, unchanged v12 policy | `875bea849cb58ca4d5c31f1e322b2ea48355441d` | `dc77869d80c74bf02a9cab2a8775c27715793dd8790734085206c40a249f1927` |
| Candidate library | `7b065abb48a2b7cb4fa1a99bdf6efa53f94964c7` | `7b6a73f14ea65a49b93ff2c443970fb48895f42284a5575dcb496626c8cd7396` |
| Frozen specialist baseline | Fixed original baseline | `13cf2555da491c3687f432f93cf778b966c6d8d4a21a4f83b83005cba2b539a2` |

[The staged plan](staged-plan.json) contains the full weight map and exact settings. [Plain analysis](plain-analysis.json), [MCTS analysis](demise-analysis.json), launch manifests and exit records preserve identities, per-deck comparisons, counters, and logical input hashes. [The copied candidate build record](candidate-build.json) preserves provenance beyond `/tmp`.

All six completed JSONL files, including the two diagnostic traces, are archived with `gzip -n -9`. [The archive manifest](archive-manifest.json) records logical and compressed SHA256 hashes and verified round trips. [Archive verification](archive-verification.json) records successful reruns of both stages through the pinned analyzer with `write=False`: results and logical fingerprints were unchanged; only raw-file input paths became `.jsonl.gz` paths. The launched plan, runner, analyzer, original metadata, summaries and analysis reports were preserved. The pinned analyzer still expects the original `/tmp` build-record path; `candidate-build.json` is its exact durable copy if that path later needs restoration.
