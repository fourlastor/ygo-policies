# Material candidate coverage: UNIMPLEMENTED diagnostic

Status: **UNIMPLEMENTED**. This note records a read-only artifact/source audit on 2026-10-14. No candidate ordering, selection cap, search budget, evaluator, or policy behavior was changed. No new games were run for this audit.

The host's eight-action cap can omit a distinct whole material completion, but the inspected real traces do **not** establish that this is a current material-search bottleneck. Measure actual proposal-node coverage before implementing a diversity rule.

## Saved evidence and its limits

Counts below include only decisions belonging to the Generic candidate's seat. “Explicit material” means a `FusionMaterial`, `SynchroMaterial`, or `XyzMaterial` hint.

| Saved trace | Games | Own selection prompts | Explicit material prompts | Actual material form / menu width |
|---|---:|---:|---:|---|
| `pilot-v7-traces.jsonl.gz` | 16 | 213 | 34 | 29 Synchro, 5 Fusion; all `SelectToggle`, 1–4 choices |
| `pilot-v8-traces.jsonl.gz` | 16 | 231 | 40 | 35 Synchro, 5 Fusion; all `SelectToggle`, 1–4 choices |
| `pilot-v8-weak-traces.jsonl.gz` | 28 | 303 | 46 | 44 Synchro, 2 Fusion; all `SelectToggle`, 1–3 choices |
| `crystal-diagnostic-v17/search32.jsonl.gz` | 16 | 202 | 0 | 48 selection menus have more than eight complete `Cards` responses; none has an explicit material hint |
| `sequence-v14/diagnosis-v14.jsonl.gz` | 8 | 58 | 0 | No selection menu has more than eight complete responses; seven prompts have the broader `Release` hint |

The first three archives cover 17 roster decks, with repeated version/seed schedules; they are not independent strength samples. All 120 explicit material prompts are incremental `SelectToggle` decisions with `picks:null`, **not** the whole `Cards` subsets produced by sequential expansion. None approaches the eight-choice cap. This does not establish the same distribution inside simulated proposal branches or across all 71 decks.

Concrete zero-based trace examples:

- Fabled, `pilot-v8-traces.jsonl.gz`, seed **1072687460**, candidate seat **0**, trace **49**: `SynchroMaterial` offers three `Toggle` actions plus `Cancel`. Trace **50** offers two `Toggle` actions after the first selection. These are individual legal engine steps, not interchangeable full material plans.
- Crystal, `crystal-diagnostic-v17/search32.jsonl.gz`, seed **1109097480**, candidate seat **0**, trace **57**: `SelectCards`, hint `Other(527)`, twelve one-card completions, active chain length one. Trace **110** has nine one-card `SpecialSummon` completions. These demonstrate wide live menus, not the proposed material-subset pattern.

The v16 search and sentinel metadata explicitly use `--trace false`; their aggregate counters cannot recover omitted membership sets. Live traces also record answers after expansion/override, while fresh root search can subsequently be skipped by chain/visibility guards. They do not identify which candidate lists an actual MCTS node retained. Broader hints such as `Release` and `ToGraveyard` can include materials or unrelated costs; the audit does not classify them from a card/deck name.

## Source-grounded failure shape

`crates/ygo-policies-ocgcore/src/lib.rs::alternative_responses` preserves the original response, generates legal complete responses through the sequential engine adapter, and retains their committed policy snapshots. It keeps at most twelve responses, with bounded second-pick exploration appended after the first pass.

Its existing `second_pick_exploration_keeps_twelve_response_cap_and_fresh_run_order` test provides a concrete **synthetic** two-of-fourteen shape: `{0,1}, {0,2}, …, {0,11}, {1,2}`. In `engine/ismcts.rs::actions`, all these responses have the same `Cards` kind, so the eight-action cap retains only `{0,1}…{0,8}`. The already legal completion `{1,2}` is unreachable through that node. This proves a mechanism, not its frequency or outcome impact in real games.

Naive farthest-subset ordering is insufficient: relative to `{0,1}`, both `{0,2}` and `{1,2}` have symmetric-difference size two. An explicit common-optional-member test detects the distinction that this distance can tie.

## Proposed bounded instrumentation

Instrument **actual proposal node creation** in `ismcts.rs::simulate`, immediately around `actions`, using its already available expanded answer and final retained indices. Do not instrument every live expansion or every revisit as a new node. No additional FFI calls, engine simulations, subset generation, or hidden-state reads are needed.

Suggested counters, separate from win/latency claims:

- Explicit-material incremental `SelectToggle` nodes, and those whose eligible action count exceeds the cap.
- Explicit-material complete-family nodes (`SelectCards`/`SelectSum`, sequential picks metadata, `Cards` responses), and capped families.
- Capped complete-family nodes where an omitted legal completion of the same size and identical required-member set excludes an optional member common to all retained completions.
- Optionally, capped complete families with other/unknown hints, reported separately rather than silently classified as materials.

Use response bytes to deduplicate exactly as candidate construction does. Compare member `CardRef`s, not printed card codes; respect each member's `required` flag and reject malformed/mixed-required metadata from the special diagnostic. Exclude `Cancel` and administrative actions. Inspect at most the twelve existing complete responses; bounded membership comparisons cost O(K²M), with K≤12 and M the offered completion size. Retain at most four public prompt samples per game if examples are needed. Sampling examples must never change selection or consume rollout budget.

## Conditional minimal behavior change

Only if the diagnostic demonstrates coverage: preserve Generic first and replace **one last non-Generic retained completion** with an already offered, equal-size completion that excludes a common optional member. Require an explicit material hint and identical required-member sets. Use stable offered order for ties. Keep the eight-action cap, all supplied legal response bytes, cached committed states, and all other action ordering unchanged.

Do not synthesize arbitrary subsets, treat `SelectToggle` steps as full plans, remove a required member, change variable-size preferences, or apply the rule to discard/target prompts merely because they select several cards. This deliberately narrow gate may miss source-specific material prompts with broad hints; that is a coverage limitation, not a reason to add deck-name routing.

Proposed focused tests: the synthetic two-of-fourteen star retains one escape while keeping Generic first; a common required member never qualifies; unequal-size/mixed-required/malformed families remain unchanged; unrelated hints and incremental Toggle prompts remain unchanged; no truncation/high cap preserves every response; instrumented behavior without the ordering change preserves exact selected responses, native traces, budgets, and restoration. Any later strength comparison must use a separately frozen, fixed paired sample. No strength or speed improvement is established by this design.
