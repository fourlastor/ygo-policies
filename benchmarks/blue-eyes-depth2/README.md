# Blue-Eyes depth-two follow-up

This continues the `blue-eyes-improved` policy from commit `ca1690f`, using the
unchanged original `blue-eyes` as the fixed opponent. Both use the original
BlueEyes deck under MR5. Only one additional rule survived validation:
**avoid spending three Level 8 monsters directly on Full Armor when the
two-material Prime Photon route is available.**

On **8,192 fresh held-out pairs**, the first improved policy won
4,938 games (**60.28%**) and the new policy won
5,003 (**61.07%**). The paired gain is
**+0.79 percentage points**, 95% normal-approximation interval
**[+0.49, +1.09]**. Both versions played the same fixed original opponent.

The new policy still overlays an existing Prime with Full Armor, and still
allows Full Armor's direct three-material summon when Prime cannot legally be
summoned. The original `blue_eyes.rs` remains unchanged. The earlier
[first-iteration results](../blue-eyes-improvement/README.md) are preserved.

## What depth two means here

The tool previously supported one searched decision. `policy-bench search
--depth 2` now also searches the same seat's next eligible decision in each
root rollout. The opponent remains its fixed policy. Forced choices, card
selections, open chains and strict-mode positions facing face-down opposing
monsters are left to the pilot. After the second searched decision, both
policies finish the duel normally. Depth is measured in eligible decisions,
including battle-position prompts, not turns or an exhaustive game tree.

The second search samples hidden cards again from that branch's information
set and message history. It does not choose the best second action with the
rollout's hidden cards known. `--inner-worlds N` uses stages min(4,N), min(8,N),
N; the outer stages remain configurable independently. Depth one is the default.
Depth two currently supports native policies, excluding model/outside seats,
foresight and validation mode.

## Search evidence

The main diagnostic uses 16 games, seed 5200000, outer worlds 4/8/16, inner
worlds 8, z=1.645, strict mode, six workers, and an 8,192-decision limit.
The policy library is frozen at the first iteration.

| Mode on the same 16 deals | Wins | Playouts |
| --- | ---: | ---: |
| Plain first-iteration pilot | 12 | — |
| Depth one | 13 | 8,964 |
| Depth two | 13 | 148,440 |

Depth two searched 8,964 continuations and changed 438 second decisions. There
were no failed playouts, duel errors or capped duels. Every depth-one and
depth-two game had the same win/loss result, so this small diagnostic does
**not** establish that depth two is stronger overall. A separate two-game smoke
run used outer 2/4/8 and inner 4; its records are also retained.

The clearest actionable position is engine seed **336710608, decision 125**:
three Level 8 dragons face a defense-position Azure-Eyes. Depth two chooses
Prime Photon Dragon instead of the direct Full Armor summon. Prime takes two
materials and can then become Full Armor, retaining the third dragon. Another
example, seed **336710599, decision 189**, chooses battle instead of consuming
three Blue-Eyes for Full Armor. The smoke run also prefers Harbinger over the
direct Full Armor summon at seed **336610599, decision 516**.

## Rules tested

All development comparisons use 2,048 pairs, seed 5500000, two workers, and the
same fixed original opponent. Their baseline is the **first improved policy**,
not the original policy. Every candidate is tested independently against that
baseline, on identical shuffles, seats and tie-break seeds.

| Candidate | Before | After | Paired change, percentage points [95% CI] | Decision |
| --- | ---: | ---: | ---: | --- |
| Full Armor only over existing Prime | 59.03% | 60.30% | +1.27 [+0.62, +1.92] | Refined to preserve the fallback |
| Prefer Ancients over Sage with an established dragon | 59.03% | 58.89% | -0.15 [-0.68, +0.39] | Rejected |
| Full Armor via Prime when available; retain the direct fallback | 59.03% | 60.35% | +1.32 [+0.68, +1.96] | Retained |

The rejected rule came from seed 336710601, decision 213, where the search
normal-summoned Ancients instead of Sage. Although useful in that position,
making it a general preference did not improve the paired evaluation.

## Validation and records

The held-out run uses seed 5600000, six workers and an 8,192-decision cap.
All 8,192 engine seeds are distinct and disjoint from both this round's
search/development games and the first iteration's recorded runs. There are
zero draws, errors or capped duels across the 16,384 duels. The new rule turns
111 losses into wins and 46 wins into losses; 6,623 pairs retain identical
response digests. The complete workspace suite passes **267 tests**, with
none failed or ignored, using localhost permission for its HTTP fixture.

- Four recorded depth-one games have identical responses, digests, deviations,
  playout counts and replay records before and after the tool extension.
- The depth-two integration test runs the same bounded duel twice and checks
  identical records, positive continuation counts and zero failed playouts.
- The policy regression test checks the cheaper route, overlaying an existing
  Prime, and the legal direct-summon fallback.
- The original policy's SHA-256 remains
  `41a57727053ef7ff467b9dfb11a156a66abb5c3752d964e56a0ca13ea8385e79`.

Each run has compressed raw JSONL, a summary and metadata with the library,
deck/database fingerprints, pinned engine/Lua/script revisions, rules, seed,
worker count and decision limit. Search rows include replay records and the
visible observations where actions changed. `manifest.json` records source and
binary SHA-256 hashes. `variants/*.patch` reconstructs each policy candidate
from `ca1690f`'s improved policy. `validation.json` checks disjoint engine seeds
and gives seat-specific held-out results. The successful complete test log is
`workspace-tests.log`.

## Commands

Build and run depth two against the unchanged original policy:

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench search \
  --policies blue-eyes-improved --opponents blue-eyes --rules mr5 \
  --depth 2 --inner-worlds 8 --worlds 4 --confirm 8 --final 16 \
  --games 16 --seed 5200000 --workers 6 --limit 8192 \
  --log true --record true --output depth2-new.jsonl
```

The retained diagnostic searched the **pre-change** policy. To reproduce it
exactly, use a policy library rebuilt from `ca1690f` through `--library` with
the new benchmark executable. To reproduce the held-out comparison, preserve
that library as `before.so`, rebuild the current policy as `after.so`, then run:

```sh
target/release/policy-bench compare \
  --policies blue-eyes-improved --opponents blue-eyes --rules mr5 \
  --baseline before.so --candidate after.so \
  --games 8192 --seed 5600000 --workers 6 --limit 8192 \
  --output depth2-policy-repeated.jsonl
```

Search depth is diagnostic only; the deployed policy is still a rule-based
pilot and does not run search during ordinary play.
