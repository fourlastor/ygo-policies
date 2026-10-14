# Fast MCTS roster screen

The 32-iteration search improved aggregate results over the same plain Generic policy, but **the goal of at least 55% against every required deck remains unmet**. This was a fresh-seed development screen with only 16 games per deck; individual deck estimates are exploratory.

| Sample | Plain wins | Search wins | Paired gain | Paired normal 95% interval |
|---|---:|---:|---:|---:|
| All 71 decks, 1,136 pairs | 395 (34.77%) | 569 (50.09%) | +15.32 points | +11.95 to +18.69 points |
| Required 70, excluding Countdown, 1,120 pairs | 385 (34.38%) | 564 (50.36%) | **+15.98 points** | **+12.59 to +19.37 points** |

Across the required decks, search gained 291 wins and lost 112; 717 paired win indicators were unchanged. Draws count as non-wins (three plain, four search across the full roster). Countdown, the declared exception, fell from 10/16 to 5/16.

Required decks with an **observed win rate ≥55% rose from 5/70 to 30/70**; including Countdown, the counts were 6/71 and 30/71. Pointwise Wilson 95% lower bounds cleared 55% for **0 → 2 decks**, Cyber and Herald, each at 14/16. Those are individual intervals, not simultaneous confidence across the roster. Forty required decks remain below the observed target. Neither the aggregate interval nor a 16-game deck result establishes universal qualification or generalization to unseen decks.

Largest observed gains were Fabled **2 → 12/16**, Cyber **6 → 14**, X-Saber **2 → 10**, Herald **7 → 14**, and Spellcaster **3 → 10**. The largest required-deck losses were Crystal **10 → 5**, Chain Burn **7 → 4**, and Burn/Volcanic **7 → 5** each. Remaining weakest search results were Deckout and Exodia **2/16**, followed by Chain Burn and Psychic **4/16**. Overall, 51 decks gained wins, nine lost wins (including Countdown), and 11 had unchanged totals. These selected extremes need confirmation; [per-deck.json](per-deck.json) preserves all 71 results, paired discordances, intervals and target flags.

Both arms completed exactly **1,136 unique scheduled games**, with 16 per deck and eight per candidate seat. Independent checks reproduced every summary and the saved comparison, with matching paired keys, metadata arguments and frozen-source fingerprints. Errors, live caps, failed searches/playouts, unresolved validation, incomplete worlds, budget exhaustion and proposal cutoffs were all zero. Search completed all **1,353,984 proposal simulations**, visited all **156,371 retained root actions**, and accepted a proposal at 7,557 of 42,312 searched decisions. The 293,832 truncated validation playouts reflect the configured heuristic horizon, not failed games or terminal proofs.

The arms ran serially with **16 workers on CPUs 8–23**, using base seed **2485000**, unchanged default weights and identical host/library/baseline inputs. MCTS used 32 iterations, four proposal and validation worlds, eight candidate actions, 1,024 nodes, 384 decisions, one reply, best-only validation and no wall-time cap. The opponent's deck composition was known; sampled hidden states used the Generic rollout model. This does not test play against an unknown deck composition.

Loaded searched-decision latency was median **254 ms**, mean **284 ms**, p95 **577 ms**, maximum **1,826 ms**. These 16-worker timings are not a single-game latency guarantee; the separate [serial performance diagnostic](../perf-v17/report.md) remains a small Demise-only measurement.

Frozen host source **62614aedd48a4cbaa17695b179d9bc68592160fb** and candidate-library source **9bacb1307f988af737c788ea844cfdc4cd902ac5** are recorded with exact build commands, artifact SHA-256 hashes and match commands in [plan.json](plan.json) and [manifest.json](manifest.json). Only enabling search changed between arms. [independent-analysis.json](independent-analysis.json) records the full-sample audit. Original plans, metadata and analyses remain unchanged; raw inputs are archived as deterministic `.jsonl.gz` files, with round-trip hashes and equivalent compressed-input comparison results recorded in the archive manifest and verification files.
