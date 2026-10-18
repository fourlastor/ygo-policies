# Demise opponent-model diagnostic

Using the actual Demise specialist as the simulated opponent increased observed wins from **13/64 to 20/64**, a **10.94 percentage-point** gain. The paired 95% interval is **−2.62 to +24.06 points**, and the exact two-sided discordant-pair test gives **p = 0.1671**. These data show an observed improvement but do not yet establish a reliable long-run gain. All variants remain below the 55% target.

| Variant | Wins / games | Win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Generic simulated opponent — primary reference | 13/64 | 20.31% | 12.27–31.71% |
| Specialist simulated opponent — primary candidate | 20/64 | 31.25% | 21.23–43.39% |
| Plain Generic, no search — later control | 13/64 | 20.31% | 12.27–31.71% |
| Opponent UCT in proposals — separate exploratory arm | 15/64 | 23.44% | 14.75–35.13% |

The primary paired outcomes are: both win **7**, Specialist only **13**, Generic only **6**, neither wins **38**. Intervals for paired differences use [Newcombe method 10](https://www.statsmodels.org/dev/generated/statsmodels.stats.proportion.confint_proportions_paired.html); the repository analyzer's approximate paired-normal interval is also retained (−2.24 to +24.12 points for the primary comparison). The exact test conditions on the 19 discordant pairs. Arm Wilson intervals use z=1.96.

The original [manifest](manifest.json) required all **64 matched games per arm**, 32 per candidate seat, before analysis, with no result-based stopping. Both arms completed. CLI seed 2472000 produces engine seeds 110470259–110470322. Both use the same frozen executable, candidate library, actual specialist opponent, cards, decks, scripts, and rules. `analyze.load` validated complete schedules, outcomes, metadata, and telemetry; the comparison permits only `search.opponent_model`. Source is `598b3cd1c464e52d9461ebbce95f7e6a3cf5fe5d`; the candidate library source is `875bea849cb58ca4d5c31f1e322b2ea48355441d`. SHA-256 fingerprints and exact commands are in the manifest.

This is a benchmark-only modeling diagnostic. The Generic candidate still chooses its own actions; the specialist is used only as the simulated opponent in proposals and fresh validation. Production has no deck-to-specialist dispatch. Both models assume known deck composition, sampled hidden arrangement and future randomness, and reconstruct opponent memory from spectator-visible history rather than copying the actual opponent's private memory. Demise was selected as a weak, stateless specialist matchup; this does not qualify performance on unknown decks or the full 71-deck roster.

## Search work and latency

All arms have **zero duel errors, match caps, failed searches, or failed playouts**. Search arms also have zero MCTS decision cutoffs, unresolved playouts, incomplete worlds, exhausted budgets, and soft deadline overruns. Completed MCTS simulations include heuristic-horizon returns and do not imply terminal simulations.

| Search variant | Searched decisions | MCTS simulations | Validation playouts | Truncated validation playouts | Search mean / p95 |
|---|---:|---:|---:|---:|---:|
| Generic model | 860 | 110,080 | 6,880 | 4,589 | 630 / 1,000 ms |
| Specialist model | 1,236 | 158,208 | 9,888 | 6,943 | 718 / 1,183 ms |
| Opponent UCT | 1,038 | 132,864 | 8,304 | 5,318 | 619 / 1,033 ms |

Each searched decision has the same configured 128 proposal iterations, four sampled proposal worlds, at most eight actions, 1,024 nodes, and 384 decisions per simulation; validation uses four fresh worlds, horizon 1, and a 256-playout cap. `millis=0` imposes no time deadline. The arms use four workers on CPUs 8–23. Different game trajectories cause different total search work. Latency is descriptive under shared and changing system load, not a controlled speed comparison. **860/860** Generic-model searches and **1,235/1,236** Specialist-model searches exceeded 200 ms. Their all-decision p95 values were 628 and 755 ms; the speed goal is unmet. Search skipped unknown facedown situations 67 and 141 times respectively.

## Additional controls

The later no-search control uses the same 64 keys, binary, library, and effect metadata. It changes search presence (`plain=true`, `max_playouts=0`) and uses two workers. Generic search versus plain improves nine pairs and regresses nine: **0.00 points**, paired 95% interval **−13.39 to +13.39**, exact **p=1.0**. Specialist-model search versus plain improves 13 and regresses six: **+10.94 points**, interval **−2.62 to +24.06**, **p=0.1671**. These are explanatory post hoc comparisons, kept separate from the original primary test.

The separate [opponent-UCT manifest](adversarial-manifest.json) changes only `search.mcts.adversarial_opponent`, sharing the same total proposal budget between actors. The opponent minimizes root-relative value during proposals; frozen validation still uses Generic. Its 64 games improve four pairs and regress two against the Generic-model reference: **+3.13 points**, paired 95% interval **−5.25 to +11.66**, exact **p=0.6875**. Opponent branching was exercised: 64,004 opponent nodes, 329,588 opponent actions, and 126,484 opponent tree hits. This exploratory result is not evidence of a reliable gain, and the sampled-world opponent is not an information-fair unknown-opponent model. No multiplicity adjustment is applied to additional comparisons.

## Default behavior and byte telemetry

The four-game v11 control **fails the requested strict non-timing parity check: 2/4 games match**. The other two games differ only in 47 snapshot entries across `max_snapshot_bytes`, `restored_bytes`, and `snapshotted_bytes`: each maximum increases by 1,536 bytes, with aggregate byte changes equal to 1,536 times unchanged copy counts. The audit removes only elapsed timings and the four checked additive v12 fields; it retains these byte differences. A separately labeled behavior diagnostic matches **4/4 games**, including outcomes, duel digests, decisions, responses, deviation gains, search counters, sample counts, and copy counts.

A repeat of those four games with the **same frozen v12 executable and library** also fails strict parity in one game, with 14 snapshot entries decreasing by the same 1,536-byte amount per copy. All other retained fields again match 4/4. This establishes within-build snapshot-byte variability; its allocator or process-level cause is unproven. It does not turn the failed strict checks into passes or invalidate the separately validated, same-binary primary comparison.

## Reproduction and evidence

Run `python3 analyze_diagnostic.py`, `python3 analyze_controls.py`, and `python3 analyze_adversarial.py` from this directory. They require the full completed inputs and verify those inputs remain byte-identical during analysis. Each accepts raw `.jsonl` or archived `.jsonl.gz`, verifies equality if both exist, and fingerprints the logical decompressed bytes. Python bytecode writes are disabled.

- [Primary comparison](comparison.json) and [paired statistics, telemetry, and input hashes](paired-statistics.json)
- [No-search controls](secondary-comparisons.json)
- [Exploratory opponent-UCT comparison](adversarial-comparison.json) and [statistics](adversarial-statistics.json)
- [Strict v11 default-parity audit](default-parity.json) and [same-build replay audit](allocation-replay-audit.json)

Raw games, metadata, summaries, and manifests were preserved unchanged. These findings support further diagnosis; they do not meet the requested strength or latency targets.
