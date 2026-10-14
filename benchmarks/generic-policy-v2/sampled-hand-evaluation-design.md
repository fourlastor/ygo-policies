# Sampled opponent-hand evaluation — UNIMPLEMENTED

This document proposes a bounded, hand-written Rust evaluation ablation. No
sampled-hand reward, new weight, training path, or strength claim is implemented.
The separate facedown-banished search guard described below is implemented.
Its three focused tests passed within 87 engine unit tests; the proposed reward
change remains unimplemented.

## Existing information boundary and the discovered gap

Known opponent deck composition is accepted for this task. The relevant
`play_fast` paths require `strict=true`, skip active chains and unknown opposing
facedown monsters, and reject unsafe pending selections before starting fresh
MCTS or beam search. `PublicIdentityMemory` learns only from spectator-redacted
messages. It forgets hidden hand/deck transitions, set-card shuffle permutations,
deleted/empty slots and reset events. An initially fixed facedown monster is
therefore publicly known at an eligible root; a newly set monster in a rollout
comes from that sampled continuation. The guard does not certify arbitrary
standalone callers of the planner or evaluator.

`HiddenWorlds::restore` restores the root, samples our unknown Deck order and the
opponent's Deck/Hand/facedown Spell/Trap arrangement, and uses canonical card
ordering before shuffling. The same seed is independent of the original hidden
arrangement within those supported pools. Known slots are excluded. Unsupported
Spell/Trap placements fail rather than constructing an illegal world. The
opponent policy is rebuilt from public history, then refreshed with the sampled
native field; later native messages update both simulated seats. Future engine
randomness is independently reseeded. These are constrained samples, not a
Bayesian posterior inferred from the opponent's previous decisions.

There is an existing concealed-banished gap independent of any reward change:

- **Destiny HERO - Doom Lord, 41613948**, in `Destiny HEROes.ydk`, can select a
  facedown opposing monster, remove it with `tc:GetPosition()`, and return it on
  its second subsequent own Standby Phase. See
  `vendor/CardScripts/official/c41613948.lua:33–65`.
- **Slip of Fortune, 72885174**, in `Fortune Ladies.ydk`, can temporarily remove
  its own facedown attack target and return it next Standby. It passes position
  zero to `Duel.Remove`; `field::send_to` preserves the current position for
  that argument. See `vendor/CardScripts/official/c72885174.lua:21–46` and
  `vendor/ocgcore/operations.cpp:325–330`.

Both are among the frozen 951 active main/extra card identities. A read-only
scan of those scripts found these concrete position-preserving paths; this is
not a proof that arbitrary future scripts have no other concealed-zone effects.
The native swap API does not support REMOVED or facedown monsters.
`HiddenWorlds::hidden` never inventories REMOVED, the previous strict guard
checked MZONE only, and public memory deliberately does not retain facedown
banished identities. A later root could therefore simulate the return of an
actual, unsampled hidden card even with the original redacted leaf evaluator.

The narrow fix skips a **fresh** root whenever the redacted observation contains
an opposing facedown Banished card, using only controller/location/face_up. It
also rejects missing or malformed relevant metadata. The exclusive counter is
`search_skipped_facedown_banished`, after the existing unknown-MZONE guard and
before unsafe-selection. Own facedown banished cards and opposing face-up
banished cards do not block search. This deliberately skips even historically
revealed opposing cards because memory does not certify their current hidden
identity. Existing exact visible-prompt continuation matching stays before the
guards and is unchanged. New tests cover both seats, private-code substitutions,
malformed metadata, and the temporary-banish wire transition. All three passed;
no native Doom Lord/Slip duel fixture is claimed.

This implementation is scoped to the new generic `play_fast` search paths
(rollout, beam and MCTS). The older diagnostic `engine/search.rs` entry point
still has its previous monster-only guard and is not fixed by these changes.

The known Extra Deck multiset is accepted composition information, and the
current evaluator assigns no direct Extra/Banished value. Do not add hidden
Extra order or concealed-zone identities to the proposed reward. With the new
root guard, later concealed cards can arise from sampled/known state inside a
continuation; their provenance must not be confused with an unrestricted live
opponent observation.

## Proposed narrow reward change

Keep `Evaluator::value`, `features`, their existing root-view redaction, and all
public battle calculations unchanged by default. Add an explicitly enabled
host-only sampled-hand path. Never average the two existing difference values:
that halves private feature weights, and averaging their post-`tanh` values
changes the nonlinear score even further.

The first ablation should replace only the opposing **Hand contribution** to
three existing features:

1. `stranded_hand`: printed tribute requirements and the current full-backrow
   restriction, preserving separate non-ACTIVATE hand effects.
2. `effect_access`: the same effect categories, costs and exact known hand
   requirements, with the existing total per-side cap applied after adding hand
   and unchanged public board contributions.
3. `alternate_win_progress`: distinct known Exodia hand pieces with the existing
   quadratic formula, if included in this first frozen definition. No new
   alternate-win rules or coefficients.

Hand card count remains the root observation's existing count. Body power,
dominant attack, grave utility, interaction, battle projection, immediate lethal,
life and Deck reserve remain exactly as computed from the root-view observation.
Do not merge opponent-private cards into that observation or its `Ctx`. Do not
read the opponent's facedown field, Banished, Extra or Deck entries to compute
the hand correction. A private helper should extract only the owner-matching
Hand entries from the simulated opponent seat and verify the public hand count.
Factor the existing hand contribution into a shared helper so the original
root-hand math and the new sampled-hand math use the same rules. Retain the
uncapped public access subtotal long enough to recompute the existing side cap;
adding a correction after the cap would be incorrect.

Only immediate Hand effects use an optional actual Idle menu. Obtain it from
the current simulated responder using `legal_idle_menu` with that seat's own
observation. Apply it only to the hand contribution being evaluated. Retain
future trigger/Quick/Trap priors and never count choice/target multiplicity.
The root's current menu behavior stays unchanged. No opponent menu enters
policy memory, information-set keys, frozen continuation conditions, or live
action selection directly.

Use an internal provenance-bearing input, not a public `value_pair` API accepting
arbitrary JSON. Construct authorization for this mode only after the strict
fresh-root guards and successful `HiddenWorlds::restore`/`prepare`; carry it with
the sampled world into its simulated seat forks. Standalone/native fixtures
without that authorization retain the old evaluator. At the leaf, require the
other observation's `me`, turn, phase, public hand count and public life points
to agree with the root view before extracting its Hand entries. A mismatch is a
failed simulation, not a silently scored partial world. Serialization can then
be reduced later without relaxing this boundary.

The exact host call points are the heuristic horizon in `ismcts::simulate`,
the horizon leaf in `planner::Search::advance`, and its quiescent beam-node
score. Fetch the additional seat observation only at those points, after the
whole native message batch has updated both seats. Terminal results stay native
`-1/0/+1`, with no extra observation read. The same configured callback must
serve proposal and fresh paired validation in both `choose_mcts_measured` and
`choose_plan`. Existing public wrappers and outer restoration retain their
contracts on read/evaluation errors.

## Expected scale, limitations and alternatives

There is a real structural asymmetry, but its sign is mixed. At default weights,
one omitted opponent draw/search option contributes `0.35 * 0.45 = 0.1575` raw
score, approximately `0.025` value near an even position. One omitted stranded
opponent card contributes `-0.65 * 0.5 = -0.325`, approximately `0.051` in the
opposite direction. The entire effect-access cap is `3 * 0.45 = 1.35` raw score.
Four distinct opponent Exodia pieces contribute `16/5 = 3.2`. These are formula
effects, not measured win-rate changes; a change common to all sibling actions
largely cancels. The informative cases are actions that draw, discard, search,
set, congest the backrow, or change the value of retained hands.

Per-world reward may depend on a sampled hidden state without revealing the
actual hidden state. Root action choice and information-set keys must remain
independent of that world's private identities. This does not fix existing
determinization/strategy-fusion approximations: beam proposals still optimize
within a sampled world, small MCTS world sets remain noisy, and uniform
composition sampling does not infer hand likelihood from earlier policy choices.
Fresh common validation worlds and frozen executable continuations remain
necessary. Full two-self-side evaluation would additionally change facedown
board and combat estimates, so it is a broader ablation than this proposal.

Current effect profiles come only from the searcher's deck. This is sufficient
for the required mirrors. In non-mirror known-composition use, opponent cards
without profiles still receive weaker estimates; adding their profiles must be
an explicit evaluator-only change, not an accidental opponent-pilot upgrade.

An analytical alternative estimates the opponent hand from the known remaining
multiset and public hand size. Additive card features admit simple weighted
expectations; distinct Exodia pieces and the access cap require joint/hypergeometric
or small sampled calculations. That can reduce variance and avoid a second FFI
observation, but it must condition on legal set-card placement and public reveals.
A static root expectation would miss the action-dependent hand changes already
present in native rollouts. Reusing the sampled hand is the narrower first test.

## Required checks before a bounded ablation

- Default behavior and every non-hand feature remain exactly unchanged. Equal
  public states/hand facts produce equal values; replacing an unknown opponent
  hand changes only features 1, 6 and 12, including cap/stranding edge cases.
- Private substitutions in forbidden field/Banished/Extra/Deck entries cannot
  affect the hand correction. Missing provenance uses the old path; malformed
  authorized input fails. Both viewer seats and actual Idle-menu ownership are
  covered.
- Native tests compare sampled own-hand facts with the simulated seat after
  draw/discard/set operations, verify proposal and validation both receive them,
  and preserve terminal callbacks and caller restoration on errors/caps.
- Differential tests hold public history, known composition and world seeds
  fixed while permuting the live supported hidden arrangement. Sampled hands,
  leaf values, root statistics, frozen choices and final restoration must match.
  Unknown initial opposing MZONE/Banished roots must skip without private reads.
- Freeze one enabled/disabled definition with identical default weights, world
  schedule, budgets, opponent policy and seeds. Compare complete paired outcomes,
  errors/caps, skip counts and latency; do not score partial results. The added
  cost is one opponent observation serialization/parse per heuristic leaf plus
  hand-only work, not another full battle evaluation. Its wall-time effect is
  unmeasured.

Recommendation: fix the concealed-banished boundary independently, then run a
bounded hand-only ablation if the current source freeze permits it. Do not claim
strength or safety for unguarded full opponent-self evaluation from this audit.
