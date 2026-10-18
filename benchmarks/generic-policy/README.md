# Experimental generic policy

This branch implements a deck-independent pilot and a sampled engine search.
**The requested 55% win rate against every specialist was not achieved.**
The 71 registered decks are evaluation fixtures, not strategy dispatch keys.
The generic implementation never invokes a specialist strategy; both simulated
players use the generic policy. Original specialist behavior is preserved.

## Use with any deck

`ygo_policies::registry::create_seeded("generic", db, seed)` and
`ygo_policy_create_seeded("generic", ...)` create the ordinary, engine-independent
pilot. It needs only the usual redacted messages and card database. Supply any
deck through the host. `generic` is absent from the 71-entry deck catalog because
it has no prescribed deck. No opponent deck list is required for this pilot.

The search needs engine snapshots and therefore runs in the host:
`ygo_policies_bench::engine::play_fast` is a public Rust library API. It takes two
arbitrary `Deck` values and a `FastOptions` budget. The example uses two `.ydk`
paths directly, with no catalog lookup; seat 0 searches and seat 1 uses the same
plain generic pilot:

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
cargo run --release -p ygo-policies-bench --example generic_decks -- \
  /path/to/your-deck.ydk /path/to/opponent-deck.ydk 123
```

The ordinary FFI policy does not silently run search. Applications wanting the
search must integrate the engine-hosted API and its compatible core.

## Algorithm and information boundaries

The pilot uses printed card properties, shared battle/staple rules, public card
facts, legal prompts and a bounded fallback for unfamiliar effects. It contains
no deck identification, per-deck parameters, or roster specialist fallback.
Unknown effect semantics and long combos remain difficult.

At an eligible prompt the search snapshots the engine and samples hidden card
arrangements from the remaining deck inventories. It tries the pilot's action
and a bounded, diverse set of legal alternatives in the same sampled worlds.
Only complete paired worlds influence the result. A tie retains the pilot.
Hidden set cards respect ordinary backrow versus Field Zone card types.
Target/material responses are generated as bounded legal subsets, with matching
policy-state snapshots; this is not exhaustive subset enumeration.

`--depth 2` additionally compares up to three continuations at the next meaningful
own prompt in each sampled rollout, including costs/targets inside that already
sampled chain. This is two-step determinized Monte Carlo lookahead, not a full
information-set MCTS implementation. Maximizing an inner value separately in
each sampled world can be optimistic about decisions with hidden information.

Search currently assumes **known opponent deck composition**. It randomizes the
unknown arrangement and future engine randomness; it does not use the actual
opponent policy for rollouts. The opponent model is reconstructed from public
history, and remembered revealed cards stay fixed. This assumption is acceptable
for the requested mirror evaluation but is not an unknown-deck-list solution.

The sampler cannot safely replace an opponent's face-down monsters, so the live
policy skips search while any is present. It also skips active-chain roots and
unsafe hidden-card selection roots. Inner lookahead may explore chain prompts
because that world was sampled before the chain. These are important strength
limitations, not merely speed optimizations. Disabling strict sampling is rejected.

The native RNG bridge is compiled against a build-local copy of the pinned core
header; it leaves `vendor/` untouched. Snapshots restore live RNG after simulated
coin/dice/shuffle events. External cores must provide the reseeding extension to
run search.

## Budgets and benchmark commands

Freeze a specialist library built from the starting revision before rebuilding
the candidate. The experiments here use baseline commit `7ddad06`.

```sh
policy-bench generic --policies all --plain true \
  --baseline /path/to/frozen/libygo_policies.so \
  --games 64 --seed 2420000 --workers 6 --limit 8192 \
  --output plain.jsonl

policy-bench generic --policies all \
  --baseline /path/to/frozen/libygo_policies.so \
  --games 64 --seed 2420000 --workers 6 --limit 8192 \
  --worlds 4 --max-candidates 8 --max-playouts 128 --depth 2 \
  --horizon 0 --millis 200 --output searched.jsonl
```

Both sides use the specialist's same deck, with alternating seats and identical
seed schedules between configurations. There is one generic policy for every
matchup. Only Countdown is exempt by default, and its results remain visible.

`--millis 0` disables the wall-clock budget for deterministic work limits.
`--horizon 0` rolls to termination. A positive horizon requests a public-board
evaluation after that many future decisions, delayed to an unchained main/battle
prompt. Rollouts that cannot reach a stable evaluation promptly are discarded.
`--max-playouts` bounds outer and inner simulation attempts together; depth 2
needs more of this budget to complete a sampled world.

The time budget is **soft**: a single engine call, snapshot, or policy operation
cannot be interrupted. Wall-clock-limited choices may depend on machine load.
Reported latency includes policy choice and search, not the surrounding game
engine processing; p95 across all decisions also includes forced/simple prompts.

Reports record raw wins (draws are not wins), per-deck Wilson 95% intervals,
seat balance, failures, caps, search failures, coverage, and decision latency.
Missing/duplicate/unexpected records and failures prevent target qualification.
Observed 55% and a 95% lower bound above 55% are separate fields; individual
intervals are not simultaneous confidence guarantees over 71 matchups.

Each run has adjacent metadata and summary files. The metadata fingerprints the
executable, policy libraries, card database and decks, and records engine/script
revisions, seeds and budgets. Compressed `.jsonl.gz` files retain the raw rows;
decompress them for tools expecting JSONL.

## Results

The initial no-search development screen (`plain-screen`, seed 2410000) completed
2,272 games (32 per deck): 664 wins, **29.23%**, with no errors, draws, or caps.
That small screen establishes a starting point only.

`search-smoke-v1` is a retained **invalid plumbing diagnostic**: all attempted
rollouts failed because a field refresh cleared the pending-response checkpoint.
The bug was fixed before later runs. Do not use that file as strength evidence.

All pre-fix searched smoke runs are now marked invalid diagnostics in adjacent
`.status.json` files because of the Field Zone sampling defect. Their historical
mechanical summaries must not be interpreted as valid strength evidence. They
also span implementation revisions and therefore do not isolate search depth. `heldout-fast` and `heldout-depth2` were stopped after review found
a Field Zone sampling defect: they are invalid diagnostics, with explicit
`.status.json` files, and must not be used as strength evidence. Their matching
`heldout-plain` run remains a valid measurement of the earlier plain pilot.

The final pilot additionally accepts engine-legal alternate summon procedures
when printed Level would incorrectly suggest unavailable tributes. Final
validation is recorded separately with new fingerprints and fresh seeds.

See [the final evaluation](RESULTS.md): the 200 ms configuration won 179/568
mirrors (31.51%), versus 181/568 (31.87%) for the plain pilot. The larger-budget
four-deck search won 14/32 (43.75%), with no statistically established gain.
The requested strength target remains unmet.
