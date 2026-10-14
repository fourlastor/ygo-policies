# Generic policy: planning and MCTS experiments

Work on `codex/fast-generic-policy` continues. The 55% per-specialist mirror target
has **not** been achieved. These are development experiments, not a final held-out
qualification. The 71 decks are fixtures; there is no deck-name dispatch in the
policy. No machine learning, fitted coefficients, model files, or training path
is used. Policy decisions and planning are Rust code; the existing OCGCore/Lua
engine supplies legal actions and simulates card rules.

Card-knowledge work is scoped to the original 71 decks: 951 distinct main/extra
cards and another 17 cards occurring only in side decks. This is the coverage
scope, not a claim that every card's strategic use is understood. Changes being
developed after a run are not evidence about that run's frozen candidate.
The v6 card-knowledge audit confirmed 45 active card identities with at least one
explicit contextual rule: 18 general
knowledge entries, 16 alternate-condition entries and 11 conversion entries.
All belong to those 951 main/extra cards; the 17 side-only cards remain outside
the current active-card scope. This partial knowledge was added after the frozen
v4 experiments below, so their results cannot validate it.
The [v7 coverage inventory](card-coverage-v7.json) raises this to 65 identities
with explicit partial rules within the same 951-card active scope. Registry sizes
are 18 general knowledge, 16 alternate conditions, 11 conversions, 17 requirements,
four transactions and two material rules; overlaps produce 65 distinct cards.
Initial action metadata exists for 822 cards: 751 have a categorized action and
71 have only category-zero actions. Shared lookup facts cover 270 active cards.
These overlapping counts describe available facts, not complete card semantics
or measured playing strength.

## Changes from the first experiment

- Initial card-effect descriptors are extracted in isolated, unstarted engine
  instances. Range, event, category, cost presence and description inform shared
  timing/material rules. These declarations do not prove costs or conditions.
  Enriching the pilot uses its own deck definitions, without an opponent list.
- A transaction-complete beam explores costs, targets and materials. It prunes
  only at resolved Idle/Battle boundaries, with fixed hand-written evaluation.
  Promising prefixes receive complete generic continuations.
- The beam constructs proposals in a separate sampled world, then evaluates the
  **same executable plans** on common validation worlds. An unchanged generic
  control is always candidate zero. Optimizing the control's continuation makes
  a separate candidate. Ties preserve the unchanged control.
- Live continuations require an exact original visible prompt and matching legal
  action/response. They run before active-chain search guards. A mismatch drops
  the plan. This fixes the former depth-two mismatch between assumed material
  choices and subsequent actual play.
- Information-set MCTS shares a tree across sampled hidden worlds and records a
  conditional continuation. A frozen proposal is compared with unchanged generic
  play on fresh common validation worlds. The reported v4 runs validate only the
  root action selected by MCTS, which remains the default. Experimental
  `--mcts-validate-roots true` also validates retained, measured root actions.
- Search still assumes known opponent deck composition, uses a generic opponent
  model, and skips roots with unsampleable opposing face-down monsters or unsafe
  hidden selections. Existing native swap semantics do not safely support
  arbitrary face-down monsters; this remains an important coverage limitation.

## Development evidence

The frozen specialist library is built from `7ddad06`. Metadata beside each run
records binary fingerprints, engine/scripts, decks, seeds and settings.

| Comparison | Plain/control | Candidate | Paired difference, 95% interval |
| --- | ---: | ---: | --- |
| Effect metadata, 16 games on each of 71 decks | 329/1136 | 347/1136 | +1.58 points [-0.16, +3.33] |
| Beam with heuristic validation, four decks, 8 games each | 9/32 | 12/32 | +9.38 points [-11.00, +29.75] |
| Revised generic rules, same 71-deck development schedule | 347/1136 | 363/1136 | +1.41 points [-0.53, +3.34] |
| Revised pilot plus heuristic beam, four decks | 10/32 | 13/32 | +9.38 points [-12.82, +31.57] |
| Original metadata pilot plus terminal beam, four decks | 9/32 | 13/32 | +12.50 points [-8.60, +33.60] |
| v4 generic pilot versus v3, same 71-deck schedule | 363/1136 | 369/1136 | +0.53 points [-0.07, +1.13] |
| v4 MCTS, four decks, 8 games each | 7/32 | 14/32 | +21.88 points [+2.73, +41.02] |
| v4 MCTS, all 71 decks, 4 games each | 89/284 | 129/284 | +14.08 points [+7.73, +20.43] |
| All-root validation versus best-only MCTS, four decks | 14/32 | 11/32 | -9.38 points [-31.57, +12.82] |
| Larger MCTS budgets versus smaller all-root MCTS, four decks | 11/32 | 19/32 | +25.00 points [+7.40, +42.60] |
| v6 generic pilot versus v4, same 71-deck schedule | 369/1136 | 397/1136 | +2.46 points [+1.47, +3.46] |
| v6 MCTS versus v4 MCTS, same 71-deck schedule | 129/284 | 134/284 | +1.76 points [-1.08, +4.60] |
| v7 generic pilot versus v6, same 71-deck schedule | 397/1136 | 398/1136 | +0.09 points [-0.21, +0.39] |

Intervals are paired normal 95% intervals over the exact deck/seed/seat schedule.
The earlier pilot and beam intervals include zero. The small MCTS smoke has only 11
discordant pairs (9 improved, 2 regressed); its exact two-sided paired binomial
test gives p=0.065, so its positive normal interval is fragile. The larger MCTS
roster comparison has 65 improved, 25 regressed and 194 unchanged win indicators.
It supports aggregate improvement on this development schedule, not a per-deck
55% guarantee. The candidate won 45.42% overall, or 129/280 (46.07%) excluding
Countdown. Only 18/70 required decks met the observed target, versus 8/70 for
plain play; none had a Wilson lower 95% bound above 55%.

With only four games per roster deck, individual deck gains remain uncertain.
The largest observed gains were Cloudian, Dragunity and Gemini, each 0/4 to 4/4;
the largest losses were Rock Block, 3/4 to 1/4, and Volcanic, 4/4 to 2/4.

All-root validation won 11/32, versus 14/32 for best-only and 7/32 for plain play.
Against plain play its paired difference was +12.50 points [-12.00, +37.00].
Neither comparison establishes an improvement or regression. The candidate
library was unchanged, but the executable changed from v4 to v5, so these are
explicitly labeled revision comparisons rather than isolated setting ablations.
Best-only validation remains the default; all-root validation is experimental.

The larger-budget diagnostic won 19/32 (59.38%): Blackwing 5/8, Heroes 4/8,
Machina 6/8 and Monarch 4/8. It improved nine paired outcomes and worsened one
relative to smaller all-root MCTS with the same executable and library. This
compares a bundle of larger budgets, not one isolated budget dimension. Against
plain play the gain was +37.50 points [+14.64, +60.36]; against best-only v4 MCTS
it was +15.63 points [-4.27, +35.52]. Those two comparisons cross executable
revisions. This four-deck development result does not meet the per-specialist
target: Heroes and Monarch remained at 50%, and the other 67 decks were untested
at this larger setting.

The v6 pilot uses binaries frozen at source revision `2961bfa`; subsequent source
changes are outside this evidence. Compared with v4, it improved 31 paired
outcomes and worsened three, reaching 34.95% overall. This is a candidate and
executable revision comparison, so the gain cannot be assigned to one new rule.
Only 6/70 required decks met the observed 55% target and none cleared it by the
Wilson lower confidence bound.
The largest observed gains were Cloudian and Exodia, each 0/16 to 8/16, and
Destiny Board, 4/16 to 11/16. Heroes was the only deck with a net decline, 6/16
to 5/16. These counts describe this schedule, not card-by-card causal effects.

With the same smaller MCTS configuration, v6 won 134/284 (47.18%) versus v4's
129/284, improving 11 paired outcomes and worsening six. The interval includes
zero. Only 19/70 required decks met the observed 55% target, and none cleared the
Wilson lower confidence bound. Agents, Cloudian and Heroes each lost two wins
out of four relative to v4 MCTS, while eleven other decks gained one each. All
games and searches completed without errors or caps. All 35,193 retained root
candidates were visited and 1,906/9,247 searched roots accepted a proposal (20.6%).
These results compare candidate revisions with unchanged effective search settings;
historical missing `validate_roots` and v6's recorded false are equivalent.

The v7 plain run, frozen at `405d163`, won 398/1136 (35.04%) versus v6's 397/1136.
Only three paired win indicators changed: Deckout improved 1/16 to 2/16,
Infernity improved 8/16 to 9/16, and Fabled declined 5/16 to 4/16. The interval
includes zero; this does not establish a strength improvement. Seven of 70
required decks met the observed target, none cleared the Wilson lower bound,
and all games completed without failures or caps.

The v6 small/large roster runs overlap, and their shared CPU affinity was reduced
during execution to logical CPUs 8–23, leaving four complete physical cores
(logical CPUs 0–7) free for the user. `cpu-affinity-intervention.json` preserves
the intervention log from `/tmp/ygo-search-cpu-limit.json`. Treat these runs as strength evidence only:
their latency is not a controlled speed comparison. These runs use fixed work
budgets and no time deadline. Future concurrent searches must share at most
16 worker threads total and use `taskset -c 8-23` on this host. While a 12-worker
run remains active, any new benchmark may use at most four workers.

The beam smoke search latency was
about 963 ms mean and 2947 ms p95 under four concurrent workers, well above the
200 ms target. It executed 1,773 saved live steps, with zero failed searches,
failed playouts or capped live games. Terminal validation scored 13/32 but cost 4103 ms mean and 9414 ms p95 search
latency before the validation-tail optimization. The revised pilot/optimized
heuristic beam also scored 13/32, with 655 ms mean and 1967 ms p95. These are
concurrent development workloads, not controlled speed comparisons. All reported
runs completed with no live errors/caps or failed searches/playouts.

The MCTS roster used 12 workers and no time deadline. Search latency was 929 ms
median, 2037 ms p95, 6088 ms maximum and 1033 ms mean; all-decision p95 was 1156 ms.
The four-deck MCTS smoke used four workers and measured 846 ms median and 2272 ms
p95 search latency. These settings are not fast. The roster's 35,164 retained
root candidates were all visited, but candidate pruning means this is not every
legal action. Of 9,280 searched roots, 1,893 proposals were accepted (20.4%).
Search skipped 2,449 eligible prompts inside chains and 5,512 with opposing
face-down monsters; unsafe-selection skips were zero. The saved conditional
policy matched 3,196 live prompts and missed 27,747. All 1,187,840 proposal
simulations completed without reported cutoffs, while 65,234/74,240 validation
trials (87.9%) used nonterminal heuristic values. Zero cutoffs does not mean
terminal-only evaluation.

The all-root diagnostic measured 985 ms mean and 1793 ms p95 search latency under
four workers. It attempted 16,588 validation rollouts across 931 searched roots,
versus 8,800 across 1,100 roots for best-only. Different game trajectories and
concurrent workloads prevent interpreting the timing difference as a speedup.
Larger MCTS budgets cost 4355 ms mean, 8219 ms p95 and 15585 ms maximum per
searched decision under four workers, with no time deadline. All 32 games and
their searches completed without failures or caps. This is a strength diagnostic,
not a fast configuration.

The v6 workspace run passed 361 enabled tests, with seven native
fixtures ignored by the ordinary suite. All seven native fixtures were then run
separately with the frozen v6 binary and passed in 402.23 seconds under concurrent
load; that elapsed time is not a performance benchmark. The native planner tests
check material-target optimization, exact continuation replay, terminal/cap
semantics, and state restoration; the host test checks two deterministic real
mirror games and replays their recorded actual responses. Fixed-validation
fast-path parity preserves outcomes, responses, failure/cap counts and plan use.
The policy library's clone operation preserves state and response while avoiding
copying disposable JSON scratch.
Separate native MCTS checks cover deterministic hidden-world search, continuation
replay, budgets, and the all-root validation path; these do not establish strength.
A subsequent v7 workspace run passed 392 enabled tests with the same seven native
fixtures ignored by that ordinary run; it does not replace the separate v6 native
validation above.

Archived completed raw runs are stored as `.jsonl.gz`; metadata and summaries retain their
original names. `archive-manifest.json` records original byte counts and SHA-256
digests, verified after decompression. Comparison JSON files point to the archives.

The first beam smoke predates terminal validation and used the turn horizon even
though its command contained `--horizon 0`. Its metadata explicitly records
`search.validation=heuristic` and this correction. Current code honors zero as
terminal validation. Do not pool these modes.

## Reproduce

Build and freeze the candidate separately from the frozen baseline. Use new
output paths; the runner rejects overwriting results. On this host, prefix each
benchmark command below with `taskset -c 8-23` and keep the sum of worker counts
across concurrent runs at or below 16.

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
policy-bench generic --baseline /path/to/frozen-specialists.so \
  --library /path/to/candidate.so --policies all --plain true --effects true \
  --games 16 --seed 2450000 --workers 6 --limit 8192 --output effects-on.jsonl

policy-bench generic --baseline /path/to/frozen-specialists.so \
  --library /path/to/candidate.so --policies blackwing,heroes,machina,monarch \
  --method beam --effects true --worlds 8 --max-candidates 6 --max-playouts 512 \
  --beam-nodes 64 --beam-width 6 --beam-actions 4 --beam-decisions 256 \
  --beam-replies 1 --horizon 0 --millis 0 --games 8 --seed 2451000 \
  --workers 4 --limit 8192 --output beam-terminal.jsonl

# Historical v4 MCTS roster configuration; use a frozen matching executable.
policy-bench generic --baseline /path/to/frozen-specialists.so \
  --library /path/to/candidate.so --policies all --method mcts --effects true \
  --worlds 4 --max-candidates 8 --max-playouts 256 --horizon 1 --millis 0 \
  --mcts-iterations 128 --mcts-worlds 4 --mcts-actions 8 --mcts-nodes 1024 \
  --mcts-decisions 384 --mcts-replies 1 --mcts-terminal false \
  --mcts-validate-roots false --games 4 --seed 2456000 --workers 12 \
  --limit 8192 --output mcts-roster-smoke.jsonl
```

The original v4 executable predates the `--mcts-validate-roots` option and always
used false. Omit that flag when replaying that exact binary; explicitly pass
false in current code to select the same validation scope. Experimental true
adds fresh-world comparisons for retained measured root actions. The four-deck
smoke uses `--policies blackwing,heroes,machina,monarch`, 6 for both candidate/action
caps, 256 MCTS decisions, 8 games per deck, seed 2455000 and 4 workers. Its plain
control uses the same frozen candidate, schedule and inputs with `--plain true`.
MCTS proposal worlds (`--mcts-worlds`) and fresh validation worlds (`--worlds`)
are separate streams. `--mcts-terminal` controls proposal returns; `--horizon 0`
independently requests terminal validation. Recorded v4 MCTS results use heuristic
proposal and validation values.

To reproduce the larger-budget diagnostic, use the four-deck smoke schedule and
`--mcts-validate-roots true`, then change proposal iterations to 512, proposal
worlds to 8, MCTS actions and maximum candidates to 12, nodes to 4096, MCTS
decisions to 384, validation worlds to 8 and maximum playouts to 768. Keep one
reply, heuristic proposal/validation, no time deadline and four workers. Its
primary comparison is against the smaller all-root run with the same v5
executable; it must remain separate from the best-only/all-root comparison.

Use `../generic-policy/analyze.py` with explicit `--allow-metadata-ablation`
paths for `search.method`, `search.mcts` and `search.validation` when comparing
plain control to MCTS. Comparing best-only and all-root validation requires
`search.mcts.validate_roots`; if the executable changed, also declare
`--allow-candidate-change`. The report records that revision difference and does
not label it an isolated configuration ablation. Historical missing
`validate_roots` is explicitly interpreted as false. Cards, decks, frozen
opponents, engine/scripts, rules and exact paired rows must still match.
For unchanged search settings across candidate revisions, including v4-to-v6
MCTS, declare only `--allow-candidate-change`; an unused metadata-ablation flag
is rejected.

For beam search, `--max-playouts` bounds attempted proposal transitions plus
validation rollouts; for MCTS it bounds proposal simulations plus validation
rollouts. A simulation can contain many decisions, so these counts do not equate
units of engine work. `--beam-nodes` is the
per-root-action proposal cap; `--beam-decisions` bounds a proposal path. Terminal
validation respects the game decision limit. A nonzero `--horizon` selects
heuristic validation at the beam's requested turn horizon. `--millis` is a soft
budget checked between engine operations. Half of its remaining time is reserved
for validation; each proposal receives an equal portion of the other half.
