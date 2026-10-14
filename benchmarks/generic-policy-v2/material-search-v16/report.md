# Megamorph and material-selection ablations

The Megamorph fix improved plain Generic on Demise, but these runs do not establish a search-strength improvement. Exploring second picks changed some trajectories without improving the measured outcomes. Single-worker search latency remains about **0.9 seconds per searched decision**, above the 200 ms target. The goal of at least 55% against each required deck remains unmet.

All comparisons pair the same deck, engine seed and candidate seat, with balanced seats. Each sample size was fixed before its stage ran. Cards, decks, specialist baseline, executable and all 13 default evaluation weights stayed fixed; only the candidate library changed between v14/v15/v16. The later plain-versus-search follow-up instead kept the v15 library fixed.

| Completed stage | Base seed / sample per arm | Wins and paired changes |
|---|---|---|
| Plain Megamorph, v14 → v15 | 2481000; 64 × 71 decks = 4,544 | Overall 1,714 → 1,722. Demise **24 → 32/64**, 9 gained wins and 1 lost win. |
| Demise MCTS, v14 → v15 → v16 | 2482000; 64 | **33 → 32 → 32/64**. v14 → v15: 8 gained, 9 lost. v15 → v16: zero changed winners. |
| Plain → MCTS, same v15 | 2482000; 64 | **25 → 32/64**, 16 gained and 9 lost. |
| Eight-deck MCTS sentinel, v15 → v16 | 2484000; 16 × 8 = 128 | **59 → 58/128**, zero gained and 1 lost. Herald 14 → 13/16; other seven decks' winners unchanged. |

The plain Demise gain was **+12.5 percentage points**, paired normal 95% interval **+3.2 to +21.8 points**. All 4,480 games outside Demise retained identical winners and trajectory digests. Within Demise, 25 digests changed. This supports the intended Megamorph correction in this sample, not broad policy strength.

The 64-game Demise search ablation gives no measured gain for either addition: v14 → v15 was −1.6 points (paired 95% interval −14.3 to +11.2), while v15 → v16 changed **six trajectory digests but no winners**. Identical observed winners do not prove equivalence; the normal interval degenerates when there are no discordant outcomes. The sentinel check changed 13 digests across deckout, fortune-lady, gishki, herald and monarch, with one lost Herald win. Its small per-deck samples do not establish a general regression or improvement.

The plain-versus-search follow-up was chosen **after reviewing the earlier ablation**, so it is a development diagnostic. Its observed +10.9-point search gain has a paired 95% interval of **−4.3 to +26.1 points**; search benefit after the card fixes is still uncertain. No weight tuning was used.

The serial latency stage used seed 2483000, four Demise games per arm and one worker, without simultaneous benchmarks or builds:

| Library | Searched decisions | Median | p95 | Maximum |
|---|---:|---:|---:|---:|
| v15 | 104 | 846 ms | 1,486 ms | 1,894 ms |
| v16 | 111 | 900 ms | 1,483 ms | 1,903 ms |

These are whole searched-decision measurements on different resulting trajectories, not aligned decision pairs or a general speed guarantee. Both arms won 1/4. Parallel strength-stage timings are not used as performance evidence.

Every completed stage, including the follow-up, had zero game errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion and MCTS proposal cutoffs. Completion and passing the development gates do not certify the strength target.

Frozen source revisions were host **5705ae27e15fa7b223f2eeb1833232fb4eb6265f**, v14 **7b065abb48a2b7cb4fa1a99bdf6efa53f94964c7**, v15 **2757223caec30f06d677dbda49c684f69fc45490** (Megamorph recipient choice), and v16 **9bacb1307f988af737c788ea844cfdc4cd902ac5** (bounded second-pick exploration). Full binary SHA-256 hashes, build records and exact commands are preserved in the [plain](plain-manifest.json), [search](search-manifest.json), [latency](latency-manifest.json) and [sentinel](sentinels-manifest.json) manifests; [followup-plan.json](followup-plan.json) records the extra control command. Frozen binary source is distinct from the launch checkout.

[plan.json](plan.json) records the schedules, default weight map and budgets: MCTS 128 iterations, four proposal and validation worlds, eight actions, 1,024 nodes, 384 decisions, one reply, 256 total playout budget, best-only validation, Generic opponent model, and no wall-time cap. The saved `*-analysis.json`, summaries and [follow-up comparison](followup-comparison.json) contain the full counts and provenance. Completed raw files are archived as deterministic `.jsonl.gz` files; original `.jsonl` references resolve to the adjacent `.gz` file. Archive manifests preserve logical/compressed hashes, and archive verification records confirm intact data and equivalent follow-up and fast32 comparisons. The separate [32-iteration diagnostic](../fast32-v16/report.md) reuses these references.
