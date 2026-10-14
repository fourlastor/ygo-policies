# Psychic and Exodia historical replay

All **32 selected historical search games replayed exactly**: 16 Psychic and 16 Exodia, eight per candidate seat, at the original base seed **2485000**. Every opponent/seed/seat key, winner, reason, turn count, decision count and trajectory digest matched the archived `fast-roster-v17/search32` evidence. The replay adds 10,988 trace entries and recorded responses; it is **not a fresh strength sample** or a comparison of a policy fix.

The original results were reproduced unchanged: Psychic 4/16 wins and Exodia 2/16. These decks were selected after observing weak roster results. No new confidence or generalization claim follows from repeating them.

The frozen host was **62614aedd48a4cbaa17695b179d9bc68592160fb**, with library **9bacb1307f988af737c788ea844cfdc4cd902ac5**. Original MCTS32 settings, default weights, cards, scripts, rules and specialist baseline were preserved. Command changes were limited to the selected deck subset, output path and `--trace true --record true`. The single 16-worker pool ran on CPUs 8–23.

All games completed with zero errors, live caps, failed searches/playouts, incomplete worlds, unresolved validation, budget exhaustion or proposal cutoffs. Independent checks reproduced [replay-checks.json](replay-checks.json), verified metadata and frozen fingerprints, and confirmed all traces were present. No tests or additional matches were run by this audit. Timing is incidental to this recorded historical replay.

[diagnosis.md](diagnosis.md) preserves the readers' specific Psychic Commander payment and first-Heart selection findings, with zero-based row/trace references and their limitations. They identify decision mechanisms; they do not establish that changing either decision would win its game or improve aggregate strength.

[plan.json](plan.json), [manifest.json](manifest.json) and [independent-analysis.json](independent-analysis.json) preserve commands, expected historical rows and provenance. After both readers finished, the raw replay was archived with deterministic gzip. Archive records verify exact logical bytes, compressed/logical hashes, unchanged trace indices and identical replay checks. Original pinned files remain unchanged; `search32.jsonl` references resolve to `search32.jsonl.gz`.
