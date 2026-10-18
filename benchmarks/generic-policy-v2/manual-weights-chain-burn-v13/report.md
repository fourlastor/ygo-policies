# Chain Burn: one manual weight preset

The manual preset won **13/64 games (20.31%)**, versus **9/64 (14.06%)** with
default weights. Six identical seed/seat games changed from losses to wins,
two changed from wins to losses, and 56 retained their outcome. The observed
paired gain is **6.25 percentage points**. Its approximate paired-normal 95%
interval is **[-2.34, +14.84] points**; the exact two-sided McNemar p-value is
**0.2890625**. With only eight discordant pairs, the exact test is particularly
useful alongside the approximate interval. This is a weak positive signal for
this preset, not an established improvement or a solution to the 55% target.

## What changed

| Existing feature | Default | Manual |
| --- | ---: | ---: |
| Accessible cards | 0.8 | 0.3 |
| Life buffer | 0.55 | 4.0 |

All other 11 coefficients, features, card knowledge and the Generic rollout
policy are unchanged. The preset was chosen before running, and only this one
preset was tried. It tests whether valuing LP more and preserving cards less
helps a burn deck spend its resources productively. In an isolated fixture where
all other features stay equal, spending one card for 800 damage becomes favorable.
That fixture establishes score behavior, not the cause of any benchmark win.
The two coefficients changed together, so the test cannot attribute the outcome
to either coefficient separately.

This adds a generic Rust benchmark option, not deck-name routing or a runtime
deck-profile calculation. Defaults remain unchanged. The same fixed weights
apply to proposal leaves and fresh validation. UCT branch selection, its
exploration constant, and all search budgets are unchanged; terminal wins and
losses still have exact values +1 and -1.

## Fixed experiment

- Same-deck Chain Burn mirrors against the frozen Chain Burn specialist.
- 64 games per arm, with 32 candidate games in each seat; both complete.
- Schedule seed 2474000; actual engine seeds 1196572947 through 1196573010.
- Eight workers per arm, 16 total, pinned to CPUs 8–23.
- MCTS: 128 simulations, four proposal worlds, eight candidate actions, 1024
  nodes, 384 decisions, one opponent reply/turn horizon, four fresh validation
  worlds, best-root validation, no wall-clock cutoff, 256 total work budget.
- Generic simulated opponent; adversarial opponent UCT disabled. Search retains
  its existing known-opponent-composition limitation.
- Same frozen executable and libraries in both arms; only the two recorded
  evaluation weights differ. No new rules or policy features were added.
- This selected weak matchup and the single manual preset are development
  evidence. They do not establish strength on unknown decks or across the roster.

Source: `5705ae27e15fa7b223f2eeb1833232fb4eb6265f`, on
`codex/fast-generic-policy`. The unchanged policy library is the frozen v11/v12
library, built from `875bea849cb58ca4d5c31f1e322b2ea48355441d`.
The specialist library is the frozen baseline from `7ddad06`.
[Manifest](manifest.json) records the exact commands, full coefficient maps,
source revisions and SHA-256 identities. [Artifact verification](artifact-verification.json)
confirms all three binaries still match the manifest. The analyzer binds the
recorded arguments, full search settings and source revision to that manifest.

## Completion and latency

Both arms had zero duel errors/caps, failed searches/playouts, proposal cutoffs,
unresolved validation and exhausted work budgets. The short-horizon validation
intentionally used heuristic leaves in 25,559/36,328 default continuations and
27,100/36,464 manual continuations; these are ordinary horizon truncations.

| Searched decisions only | Default | Manual |
| --- | ---: | ---: |
| Count | 4541 | 4558 |
| Median | 555 ms | 562 ms |
| Mean | 584 ms | 598 ms |
| 95th percentile | 986 ms | 1030 ms |

Latency was measured under concurrent 16-worker load and is descriptive, not a
controlled speed comparison. The desired 200 ms decision target remains unmet.
Wall time was 393 seconds for defaults and 365 seconds for the manual arm; the
arms ran concurrently. Changed trajectories prevent attributing runtime changes
to coefficient evaluation overhead.

The override implementation passed 36 focused Rust evaluator/API/CLI tests;
its final rejection guards were rerun, and the example compiles. All 22 analyzer
tests passed, and 45 historical metadata identities remained unchanged.
Independent source review confirmed unchanged default arithmetic and shared
proposal/validation weights.

## Evidence and reproduction

[Comparison](comparison.json) and [paired statistics](paired-statistics.json)
record the matched outcomes, approximate interval, exact paired test, complete
telemetry and input fingerprints. Raw results are archived losslessly as
`default.jsonl.gz` and `manual.jsonl.gz`; the parent archive manifest records
SHA-256 digests of their original bytes. Partial runs are rejected.

Rebuild/freeze the source revision with the exact policy libraries and use the
commands in `manifest.json` with fresh output paths. The manual command adds:

```sh
--evaluation-weights '{"accessible_cards":0.3,"life_buffer":4.0}'
```

Run `python3 -B analyze_diagnostic.py` from any directory to analyze newly completed
evidence. It refuses to overwrite existing derived reports; move the existing
`comparison.json` and `paired-statistics.json` aside when reproducing analysis,
then compare their replacements. Input evidence is never modified.

The result justifies continuing cheap manual scoring diagnostics before investing
in automatic deck-derived weights. It does not show that weight adjustment alone
can reach the target, nor does this inconclusive single preset rule that out.
