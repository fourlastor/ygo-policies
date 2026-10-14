# Crystal Release recipient diagnostic

The recipient correction **did not establish a playing-strength improvement** in either mode on the fixed 64-pair Crystal sample.

| Mode | Control wins | Candidate wins | Gained / lost wins | Paired change, normal 95% interval |
|---|---:|---:|---:|---:|
| Plain Generic | 30/64 | 31/64 | 1 / 0 | +1.56 points, −1.50 to +4.63 |
| MCTS, 32 iterations | 41/64 | 40/64 | 2 / 3 | −1.56 points, −8.45 to +5.33 |

There were 63 unchanged plain outcomes and 59 unchanged search outcomes. Trajectory digests changed in seven plain pairs and 25 search pairs. The few discordant outcomes make these estimates uncertain; neither improvement nor a regression is established. Candidate search's observed 62.5% has a Wilson 95% interval of 50.3–73.3%, so this does not qualify the 55% per-deck goal or provide all-deck evidence.

The fix handles Crystal Release's **native activation description 1068** and its beneficial Equip target polarity: a stronger opposing monster should not outrank a useful friendly recipient. Actual offered Equip targets remain the native legality authority, and existing ranking among suitable friendly targets is retained. The activation gate has no target list and uses **printed card identity** to recognize a friendly Crystal Beast; runtime-only name or set changes can therefore cause a false activation veto. This limitation is explicit in the source. Prior validation reported **41 transaction tests passing**; this artifact audit did not rerun them. Ruby, Cat, Ruins and evaluation weights were unchanged.

All four arms completed exactly **64 unique games**, 32 per candidate seat, using fresh base seed **2487000**. Samples and all four arms were fixed before outcomes, without early stopping. Independent checks reproduced both saved comparisons, schedule keys, metadata arguments, summaries and pinned fingerprints. Errors, live caps, failed searches/playouts, unresolved validation, incomplete worlds, budget exhaustion and proposal cutoffs were zero. Both search arms completed all planned proposal simulations; bounded heuristic validation is not a terminal proof.

The capacity-v18 host stayed fixed at **aea6dd818208509eb382edc69caa31893749c42e**. Only the policy library changed within each mode, from **9bacb1307f988af737c788ea844cfdc4cd902ac5** to **fb1178356392f38f535a4d15bce97ab3839854ae**. Default weights, specialist baseline, known opponent deck composition and Generic rollout model stayed fixed. Arms ran serially at 16 workers on CPUs 8–23; their loaded timings are not a latency guarantee.

[plan.json](plan.json) and [manifest.json](manifest.json) preserve exact commands, build records and binary hashes. [independent-analysis.json](independent-analysis.json) records the separate audit. All four completed raw outputs are archived as deterministic `.jsonl.gz` files; the archive manifest and verification records preserve logical/compressed hashes and confirm identical data and comparison results after compression. Original plans, runner, metadata and analyses remain unchanged; their `.jsonl` references resolve to the adjacent `.gz` files.
