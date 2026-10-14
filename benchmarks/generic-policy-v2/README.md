# Generic policy: planning and MCTS experiments

Work on `codex/fast-generic-policy` continues. The 55% per-specialist mirror target
has **not** been achieved. These are development experiments, not a final held-out
qualification. The 71 decks are fixtures; there is no deck-name dispatch in the
policy. No machine learning, fitted coefficients, model files, or training path
is used. Policy decisions and planning are Rust code; the existing OCGCore/Lua
engine supplies legal actions and simulates card rules.
Known opponent deck composition is accepted for this task. Search still samples
the opponent's hidden hand, backrow and deck order instead of reading their
actual arrangement.

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
The [v8 inventory](card-coverage-v8.json), frozen at `3faafa3`, contains 67 such
identities, adding partial rules for Honest and Black Rose Dragon. Its registries
include five transaction entries and one combat entry; other registry sizes and
the 951-card scope are unchanged. This remains partial contextual knowledge.
The [v10 inventory](card-coverage-v10.json), frozen at `9574b13`, records 117
distinct registry IDs. Applying the policy's near-alias lookup covers 119 active
passcode rows, including the alternate Honest and Black Rose Dragon passcodes.
Of these, 109 have ordinary-policy facts and 11 have search battle-evaluator
facts, with one overlapping card. All registry IDs are inside the same 951-card
pool. The inventory's 23 compiled-source fingerprints match the frozen commit;
its 71 deck files and per-card coverage totals were checked. These are overlapping
partial facts, not complete card models, and search-only facts do not improve
ordinary plain play by themselves.

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

The [fresh v17 fast-roster screen](fast-roster-v17/report.md) used 32 iterations
with the answer-reuse host and v16 policy library, default weights and 16 paired
mirrors on each of the 71 decks. Across the required 70 decks excluding Countdown,
plain Generic won 385/1,120 (34.4%) and search won 564/1,120 (50.4%): +15.98 points,
paired normal 95% interval [+12.59, +19.37]. Search reached an observed 55% on
30/70 decks, versus 5/70 for plain; only Cyber and Herald reached a pointwise
Wilson lower bound above 55%. This is development evidence with small per-deck
samples, not simultaneous qualification or unseen-deck validation. All 2,272
games completed, with zero errors, live caps, failed/unresolved searches or
proposal cutoffs. Crystal regressed from 10/16 to 5/16 and is a follow-up diagnostic.
The fast setting requires `--mcts-iterations 32`; the CLI default remains 128.

The [v17 Crystal replay](crystal-diagnostic-v17/report.md) reproduces that deck's
16 paired outcomes and trajectories with full traces. It exposed concrete
recipient and resource-availability mistakes, but is selected diagnostic data,
not another strength sample. The first follow-up,
[v18 backrow capacity](capacity-v18/report.md), removes ordinary hand activation
credit when all five main Spell/Trap slots are occupied, using the existing
stranded-card penalty. Field Spells and separate hand effects remain available.
On 64 fresh paired Crystal mirrors, control won 42 and the correction won 40:
three gains, five losses, paired interval [-11.82, +5.57] points. The logical
correction has no demonstrated strength benefit. All games completed cleanly;
default weights stayed fixed.

The [v19 Crystal Release test](release-v19/report.md) corrects beneficial equip
target polarity while leaving activation of its separate graveyard effect
unchanged. Five new tests passed as part of 41 transaction tests. In fresh
64-pair samples, plain play changed 30 to 31 wins and MCTS changed 41 to 40;
neither establishes a strength improvement. The activation gate uses printed
set membership, so runtime identity changes remain a documented limitation;
actual offered Equip targets remain authoritative.

The [v20 legal-menu experiment](legal-menu-v20/report.md) qualifies immediate
effect access with actual offered activations at the searcher's next resolved
Idle prompt. Setting a card earns no activation credit; future triggers, quick
effects and reactive traps keep their priors. This uses engine actions without
deck-name rules or new weights. The fixed three-deck comparison changed 96 to
99 wins in 192 pairs, an uncertain +1.56 points: Crystal 36 to 43, Chain Burn
16 to 16, Fabled 44 to 40, each out of 64. It therefore does not establish a
general strength gain. All 79 engine unit tests and 10 native integration tests
passed, including menu delivery, hidden-information boundaries and restoration.

The [v21 Taiyou target diagnostic](taiyou-v21/report.md) kept the v18 host and
default weights fixed, changing only the loaded policy library. On 64 fresh
Deckout pairs, plain wins changed 9 to 11 (+3.13 points, paired 95% interval
−3.00 to +9.25); MCTS32 changed 6 to 10 (+6.25 points, −1.15 to +13.65).
Neither gain is established. All games completed without errors or caps, and
45 transaction tests passed. Fresh searches covered 10.9% of control candidate
decisions and 9.9% of candidate decisions. In the candidate arm, unknown opposing
face-down monsters blocked fresh search on 32.0% of decisions and active chains
on 9.4%; cached continuations applied on 2.0%. These skip counts cover supported,
noncached prompts with multiple alternatives, with chain guards checked first.
They identify a search-coverage gap, not evidence that those opportunities can
be safely recovered or would improve wins.

The [v22 root-world warmup diagnostic](root-warmup-v22/report.md) changed wins
from 71 to 66 in 128 fresh pairs across Crystal, Gem-Knight, Gishki and Heroes
(10 gained, 15 lost; paired change −3.91 points, 95% interval −11.56 to +3.75).
All 16,186 retained candidate root actions were attempted and completed in every
supplied proposal world, but the old host lacked these coverage counters.
That coverage does not establish better decisions, and no strength gain was
demonstrated. Default weights and the v21 library stayed fixed; quality counters
were zero. Warmup remains experimental, with UCT retained as the default.

The [v14 sequencing experiment](sequence-v14/report.md) tests a separate route
from weight tuning: Demise now accounts for its LP cost and friendly collateral,
and uses profitable field wipes before developing more cards. Fresh paired plain
mirrors improved from 17/64 to 27/64 on Demise; all outcomes and digests on the
other 70 decks were unchanged. Fresh Demise MCTS improved from 23/64 to 38/64.
That 59.4% observation has a 47.1–70.5% Wilson interval and is not a qualified
55% result. The subsequent independent schedule below also illustrates its
sampling uncertainty.

The [v15/v16 follow-up](material-search-v16/report.md) isolates Megamorph target
knowledge and bounded second-pick material alternatives, with default weights
throughout. Megamorph improved plain Demise from 24/64 to 32/64; the other 70
decks again retained identical outcomes and digests. On a separate MCTS schedule,
v14/v15/v16 won 33/32/32 of 64 games. Additional material alternatives changed
six trajectories but no winners there; an eight-deck screen changed 59/128 wins
to 58/128. This establishes broader legal coverage, not a measured strength gain.
A dedicated one-worker four-game sample measured approximately 0.85–0.90 seconds
at the median searched decision: the 200 ms goal remains unmet at 128 iterations.

The [single 32-iteration ablation](fast32-v16/report.md) kept all other settings
fixed on the completed 64-game Demise schedule: 32/64 wins at 128 iterations
versus 35/64 at 32. Twelve pairs improved and nine regressed; the paired interval
crosses zero, so this is not proof of preserved or improved strength. A separate
single-worker four-game latency sample measured 190 ms median and 314 ms p95
per searched decision at 32 iterations, versus 900/1,483 ms for 128. The paths
differed and the sample is small; 200 ms is not a worst-case guarantee.

The [v17 answer-reuse optimization](perf-v17/report.md) removes repeated JSON
round trips for ordinary prompts while retaining sequential expansion and its
cached policy state. Twelve focused tests and a native differential fixture
passed; the latter removed 292 round trips across 32 identical simulations.
The same four real games retained every outcome, digest and search-work counter;
median/p95 searched-decision latency fell from 190/314 ms to 156/250 ms. Native
snapshot byte-volume telemetry differed by a small persistent offset in one
game, documented separately in the report. This is limited performance evidence,
not a strength result or a universal 200 ms guarantee.

The [v13 manual-weight diagnostic](manual-weights-chain-burn-v13/report.md)
tested one frozen Chain Burn preset on 64 paired mirrors: default weights won
9/64 (14.06%), and lower card-count/higher LP weights won 13/64 (20.31%). Six
pairs improved and two regressed; +6.25 points, approximate paired-normal 95%
interval [-2.34, +14.84], exact McNemar p=0.289. All games completed without
errors or caps. This is a weak positive signal for one preset, not an established
strength improvement; defaults and the policy library remain unchanged.

The [fresh v13 confirmation](manual-weights-chain-burn-confirm-v13/report.md)
then kept that preset frozen across 256 new paired games: default 59/256 (23.05%)
versus manual 77/256 (30.08%). Twenty-eight pairs improved and ten regressed:
+7.03 points, paired-normal 95% interval [+2.38, +11.68], supplementary Newcombe
interval [+2.32, +11.76], exact McNemar p=0.00510. All 512 games completed without
errors or caps. This is a clear positive signal for the preset on Chain Burn;
it still falls short of 55% and does not validate automatic deck-derived weights
or strength on other decks. The pilot is excluded from this confirmation result.

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
| v8 generic pilot versus v7, same 71-deck schedule | 398/1136 | 420/1136 | +1.94 points [+0.85, +3.02] |
| v6 larger MCTS budgets and all-root validation versus v6 small MCTS, all 71 decks | 134/284 | 156/284 | +7.75 points [+1.47, +14.02] |
| v8 small MCTS versus v6 small MCTS, same 71-deck schedule | 134/284 | 151/284 | +5.99 points [-0.35, +12.32] |
| v10 generic pilot versus v8, same 71-deck schedule | 420/1136 | 440/1136 | +1.76 points [+0.92, +2.60] |
| v10 small MCTS versus v8 small MCTS, same 71-deck schedule | 151/284 | 151/284 | 0.00 points [-3.39, +3.39] |

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

The completed [v6 larger-roster comparison](mcts-v6-large-roster-comparison.json)
uses identical frozen candidate and executable fingerprints on both sides. Larger
budgets **and** all-root validation improved 53 paired win indicators and worsened
31, with 200 unchanged. It scored 156 wins, 127 losses and one draw in 284 games
(54.93%). Only 27/70 required decks met the observed 55% target; none passed the
Wilson lower-bound target. Four games per deck cannot establish per-deck strength.
This bundled development comparison does not isolate which search setting helped.
All live games completed, with zero errors, live caps, failed searches/playouts,
proposal cutoffs or unresolved validation trials. All 3,913,216 proposal simulations
completed and all 31,619 retained root actions were visited; 2,483/7,643 searched
roots accepted a proposal (32.5%). Validation still used heuristic leaves in
267,441/314,096 trials (85.1%). Search latency was 5568 ms median, 12514 ms p95
and 31912 ms maximum under concurrent load and the recorded affinity changes.
Those timings are descriptive, unsuitable for a controlled speed comparison.

The completed [v8 small-MCTS comparison](mcts-v8-roster-comparison.json) preserves
the v6 small search configuration, but both candidate library and executable
revisions changed. It scored 151/284 (53.17%), with 51 improved paired win
indicators, 34 regressed and 199 unchanged. The aggregate interval includes zero.
Only 25/70 required decks reached the observed target, versus 19/70 for v6;
none met the Wilson lower-bound target. Jurrac changed from 0/4 to 4/4 and Agents
from 1/4 to 4/4. Crystal, Destiny HERO, Draconic Might and Gemini each lost two
wins. These four-game deck counts are descriptive and do not establish per-deck
improvement or regression.

V8 completed every live game with zero live caps, game errors, failed searches,
failed playouts or unresolved validation trials. It recorded one bounded proposal
cutoff: Countdown, seed 227181910, candidate seat 1. Thus 1,221,375/1,221,376
proposal simulations completed; the capped proposal returned no value and did
not update action values. All 76,336 validation trials resolved, including
66,997 heuristic leaves. All 35,961 retained root actions were visited, and
1,912/9,542 searches accepted a proposal (20.0%). Search latency was 1026 ms
median and 2225 ms p95 under changing background load. Worker counts and load
differ from v6, so these timings are not a controlled speed comparison.

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

The v8 plain run, frozen at `3faafa3`, won 420/1136 (36.97%) versus v7's 398/1136.
It improved 31 paired outcomes and worsened nine. The largest observed net gains
were Iron Chain, 2/16 to 6/16, and Jurrac, 3/16 to 7/16; Benkei and Herald each
lost one win. This supports aggregate improvement on the repeated development
schedule, not a 55% per-specialist guarantee. The observed target count remains
7/70, with no Wilson lower-bound passes; there were no failures or caps.

Spot checks in the archived v7/v8 traces show concrete behavior changes. These
examples use candidate seat 0 and zero-based trace indices:

- Fabled, seed `1072687460`: Black Rose Dragon's description-221 prompt changes
  from Yes at v7 index 72 to No at v8 index 65. Both games still end in a loss.
- Fabled, seed `1072687462`: v7 sets Nozoochee on turn 5 at index 202; v8 activates
  it from hand on turns 3 and 5 at indices 135 and 212. The outcome changes from
  loss to win.
- Worm, seed `396332351`: v7 normal-summons and bounces Honest on turns 1 and 3
  (indices 2/6 and 51/55). V8 normal-summons it at index 2 and passes the bounce
  at index 6; the outcome changes from loss to win.

Several fixes changed together and the game paths diverge, so these are behavior
checks, not isolated causal evidence that any one fix produced the extra wins.

The [v10 plain comparison](pilot-v10-comparison.json), frozen at `9574b13`, won
440/1136 (38.73%) versus v8's 420/1136. It improved 22 paired win indicators,
worsened two and left 1112 unchanged on the same 16-games-per-deck development
schedule. Both library and executable revisions changed; all fixed inputs and
recorded configuration matched. Both runs disabled search, so this result does
not validate the new MCTS proposal or battle-evaluation changes. Several plain
policy rules changed together, preventing attribution to one card rule.
Nurse rose from 2/16 to 8/16; Chain Burn, Genex and Lightsworn each gained two
wins, while Gemini declined from 6/16 to 5/16. Only 9/70 required decks met the
observed target and none passed the Wilson lower-bound target. All 1136 games
completed without failures or caps. The run used eight workers alongside a
native test, so its latency is not a controlled performance comparison.

The [v10 small-MCTS roster](mcts-v10-roster-comparison.json) also finished at
151/284 (53.17%), matching v8's aggregate score with 12 improved paired win
indicators, 12 regressed and 260 unchanged. Its paired interval is [-3.39, +3.39]
percentage points; the result does not establish aggregate improvement. Search
configuration and fixed inputs matched, while both candidate and executable
revisions changed. The observed target count remains 25/70 and the Wilson
lower-bound count remains zero. Naturia fell from 3/4 to 1/4; nine decks gained
one win each. Four games per deck cannot establish per-deck changes in strength.
All live games and 1,248,128 proposal simulations completed with zero failures,
caps, proposal cutoffs or unresolved validation trials. All 36,381 retained root
actions were visited and 1,925/9,751 searches accepted a proposal (19.7%). Search
latency was 1013 ms median and 2299 ms p95 under 12 concurrent workers; its load
and worker count differ from v8, so this is not a controlled speed comparison.

The [snapshot-copy report](mcts-v10-snapshot-metrics.json) covers all 9,751 searched
roots, each with 11 snapshots and 147 restores. The largest native image copied
within each root had a median size of 3.59 MB, p95 of 9.55 MB and maximum of
42.17 MB. Total logical payload was 470.885 GB copied into snapshots and
6294.238 GB restored, or 6.765 TB combined, averaging 693.79 MB per root. Units
here are decimal. These are counted native payload bytes, not hardware bandwidth,
total allocation or resident-memory measurements, and do not identify the share
of runtime spent copying.

The v6 small/large roster runs overlap, and their shared CPU affinity was reduced
during execution to logical CPUs 8–23, leaving four complete physical cores
(logical CPUs 0–7) free for the user. `cpu-affinity-intervention.json` preserves
the intervention log from `/tmp/ygo-search-cpu-limit.json`. Treat these runs as strength evidence only:
their latency is not a controlled speed comparison. These runs use fixed work
budgets and no time deadline. Future concurrent searches must share at most
16 worker threads total and use `taskset -c 8-23` on this host. While a 12-worker
run remains active, any new benchmark may use at most four workers.
The v7 MCTS timing also includes a deliberate pause during v8 validation, so its
elapsed latency is unsuitable for performance claims.
That v7 roster run was subsequently discontinued after 107/284 recorded games
to redirect its four workers to frozen v8. Its partial archive is diagnostic
evidence only, has no full summary, and is rejected by the comparison analyzer.
The completed [v8 validation schedule](v8-validation-scheduling.json) records the
earlier pause and successful resume before this separate termination.

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
The native host test for validated MCTS continuations and deterministic replay
also passed with the frozen v8 library (one test, 85.97 seconds under load).
The frozen v10 native MCTS replay test also passed (72.47 seconds), including
snapshot-copy metrics for each searched root. These counters measure successful
native snapshot/restore calls and logical payload bytes, not measured hardware
memory bandwidth or total process memory use.

Archived completed raw runs are stored as `.jsonl.gz`; metadata and summaries retain their
original names. `archive-manifest.json` records original byte counts and SHA-256
digests, verified after decompression. Comparison JSON files point to the archives.
Discontinued partial data is listed separately in `partial-archive-manifest.json`,
with a status document that prevents its use in completed strength comparisons.
The completed `pilot-v8-weak-traces.jsonl.gz` contains 28 selected diagnostic
games, archived after all trace readers finished. Its metadata and summary are
preserved; selection for weak-deck investigation makes it unsuitable as a new
representative strength estimate.

The first beam smoke predates terminal validation and used the turn horizon even
though its command contained `--horizon 0`. Its metadata explicitly records
`search.validation=heuristic` and this correction. Current code honors zero as
terminal validation. Do not pool these modes.

## How the current search chooses branches

The unchanged Generic move is retained first. A bounded set of legal alternatives
adds action-kind and distinct Extra Deck summon diversity. Sequential prompts
produce up to twelve complete responses by varying first picks and, if space
remains, second picks; the host still retains at most eight for these experiments.
This is a candidate subset, not exhaustive enumeration of every legal play.

By default, root and inner nodes use UCT: mean completed simulation value plus
`sqrt(2 * ln(parent_attempts + 1) / action_attempts)`. Experimental root-world
warmup first schedules a fair pass through every supplied world for each retained
root action, within the existing trial budget, before returning to UCT. With
eight actions, four worlds and 32 trials, that pass fits exactly. Failures consume
scheduled attempts but are tracked separately from completed coverage.
Search selects the root action before cycling its sampled hidden world, adds at most one useful tree node per
simulation, and uses Generic for the remainder of that rollout. The rollout
ends at a resolved turn boundary after the configured reply horizon, or at a
terminal result; failed or capped simulations do not contribute a value. Terminal
returns are ±1; other leaves use the fixed 13-feature evaluator.

The highest-mean proposal is frozen, then compared with the unchanged Generic
continuation on four fresh common hidden worlds. It replaces Generic only if
that fresh comparison is better; ties retain Generic. This is not recursive
MCTS self-play. Hidden arrangements are sampled from known opponent deck
composition, and unsafe active-chain or unknown face-down monster roots remain
outside new search. Exact cached continuations can still be replayed.

## Reproduce

Build and freeze the candidate separately from the frozen baseline. Use new
output paths; the runner rejects overwriting results. On this host, prefix each
benchmark command below with `taskset -c 8-23` and keep the sum of worker counts
across concurrent runs at or below 16.

For a controlled feature-weight experiment, beam/MCTS accepts a sparse named
override such as `--evaluation-weights '{"accessible_cards":0.3,"life_buffer":4.0}'`.
Unspecified coefficients retain their defaults; the same fixed weights apply to
proposal search and fresh validation. Metadata records all 13 effective weights.
Plain play, rollout search, zero-work search and terminal-only MCTS reject an
explicit override because they would not use it. Compare otherwise identical
runs with `--allow-metadata-ablation search.evaluation_weights`; historical runs
without recorded weights retain unknown weight configuration.

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

`mcts_cutoffs` counts proposal simulations stopped by their decision/time bounds
without a value; these returns do not update action values. It is separate from
live-game `caps` and failed searches/playouts. Report proposal cutoffs together
with attempted/completed simulations, even when all live games finish normally.

The `run_perf_v9.py` experiment compares frozen `1576a59` and performance revision
`2b289c6` using each version's own executable and FFI library. It runs two games
per version on Blackwing, Fabled, Monarch and Worm, seed 2468000, one worker,
alternating version order across decks. It uses the small MCTS settings above
with four proposal/validation worlds, eight candidates, 128 iterations, 1024
nodes, 384 decisions, 256 total proposal/validation attempts and best-only validation. The script
checks the exact frozen SHA-256 values and unchanged generic/evaluator sources.

`/usr/bin/python3 benchmarks/generic-policy-v2/run_perf_v9.py plan` prints the commands
without running them. An explicit `run` requires host process visibility and a
fresh output directory; it refuses any other benchmark except the exact four-worker
v8 roster, or accepts its validated completion. It temporarily confines active
v8 threads to CPUs 12–23 while measurements use CPUs 8–11, then restores CPUs
8–23. It verifies the completed v6 larger run before launch. Scheduling events,
original masks and child affinities are saved; SIGKILL cannot execute restoration.
The paired report requires exact outcomes, digests and all non-timing counters
before comparing decision/search latency. It measures a bundled host/library
optimization, with remaining background memory-load interference and only eight
game pairs; it is not playing-strength evidence. These small performance raw
files stay uncompressed so their recorded evidence hashes remain directly verifiable.

Use the system interpreter above on this host: its native `os.pidfd_open` support
was verified. The first attempt in `perf-v9-2b289c6` used the default local Python,
which lacked that function. It failed before changing affinity or launching any
measurement; its manifest, event log and failure status are retained separately.
The fresh retry uses `run --output-dir benchmarks/generic-policy-v2/perf-v9-2b289c6-r2`.

That retry [completed successfully](perf-v9-2b289c6-r2/README.md): all eight paired
games matched exactly on outcomes, digests and every non-timing search counter,
with zero failures, live caps or proposal cutoffs. The background v8 job ended
during optimized Blackwing, so its two game pairs are excluded from the primary
timing subset. The remaining six pairs on Fabled, Monarch and Worm had no
competing benchmark process: total search time fell from 223.255 to 211.885
seconds, **5.09% lower**, over 234 matched searched decisions. Their mean per-game
reduction was 4.88%; search p95 was 1685 to 1583 ms. These are descriptive results
from a small fixed sample, with correlated decisions and uncontrolled unrelated
system activity. They establish neither a global speed guarantee nor the 55%
strength target. Full source/binary fingerprints and the bundled host/library
scope are documented beside the results. `summarize_perf_v9.py` reproduces the
background-separated report without running matches or changing input files.
