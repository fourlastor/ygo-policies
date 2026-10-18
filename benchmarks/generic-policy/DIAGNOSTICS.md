# Generic pilot: recorded failure cases

Six plain-policy mirror games were recorded with two alternating seats per deck:

```sh
target/release/policy-bench generic --policies heroes,blackwing,infernity \
  --baseline /tmp/ygo-generic-baseline/libygo_policies.so \
  --library target/release/libygo_policies.so --plain true --record true \
  --games 2 --seed 2430000 --workers 1 \
  --output benchmarks/generic-policy/diagnostic-plain.jsonl
```

All six games completed without failures or decision caps; generic lost all six. This tiny diagnostic sample establishes examples of behavior, **not a strength estimate or a claim about the searched policy**. The [metadata](diagnostic-plain.metadata.json) identifies the builds and settings. Rows below are zero-based in the [record file](diagnostic-plain.jsonl).

1. **Recursive material spending without a payoff.** HERO row 2, seed 753810286, turn 1: generic uses Ocean and Woodsman to make Gaia, then immediately spends that Gaia and its second Ocean to summon another Gaia. The opposing field is empty; the second fusion leaves the same monster and one fewer hand card. [Replay](diagnostic-plain-2.txt), lines 6–23. Row 3, turn 2 repeats the broader pattern: The Shining is immediately consumed with another body for Gaia, and the resulting Gaia remains in Defense Position. [Replay](diagnostic-plain-3.txt), lines 35–52. Generic improvement direction: evaluate the complete activation/material/summon transaction, penalize resource-negative cycles, and value the resulting board and remaining continuation rather than legal activation alone.

2. **Setup occurs after its payoff window.** Blackwing row 0, seed 1810259038, turn 5: generic Normal Summons Blizzard, resolves its revival, then activates Black Whirlwind. [Replay](diagnostic-plain-0.txt), lines 82–97. The [raw protocol](diagnostic-protocol-0.txt), lines 506–507, confirms Whirlwind activation and the Normal Summon were simultaneously legal before the summon. Whirlwind's summon-triggered search cannot retroactively apply. Generic improvement direction: include setup-before-summon action sequences and summaries of known cards' trigger events; increasing the number of identical weak rollouts does not teach this ordering.

3. **Graveyard setup is scored like discarding a cheap card.** Infernity row 4, seed 1751983388, turn 5: Foolish Burial sends Infernity Mirage from the Deck to the Graveyard. [Replay](diagnostic-plain-4.txt), lines 61–70. The [raw selection](diagnostic-protocol-4.txt), lines 366–368, also offers Glow-Up Bulb, Plaguespreader Zombie and Stygian Street Patrol. Mirage has no Graveyard activation and cannot be Special Summoned from there, as implemented in [its script](../../vendor/CardScripts/official/c86197239.lua), lines 5–24. This is a poor setup choice, not proof that another selection wins the duel. Generic improvement direction: distinguish effect resolution from paying costs, model destination-specific utility and accessible future effects, and search selection continuations.

4. **Printed-level assumptions override legal engine actions.** Blackwing row 1, seed 1810259039, turn 8: generic draws Sirocco on an empty friendly field and ends the turn. [Replay](diagnostic-plain-1.txt), lines 120–126. The [raw protocol](diagnostic-protocol-1.txt), lines 722–723, explicitly offers its Normal Summon and Set; response 224 is EndTurn. The shared summon heuristic infers tribute count from printed level and rejects it for lacking a tribute. This concrete veto is fixed in the final pilot by accepting the offered procedure when printed tribute count exceeds available friendly monsters, with synthetic unseen-card regression coverage. The recorded diagnostic precedes the fix; it remains an example of the original defect, not current behavior.

These cases identify rollout and representation weaknesses. They do not establish that any proposed repair raises held-out win rate. The next useful experiment is a generic transaction-aware rollout policy, followed by independently seeded comparisons at the same search-work and latency limits. Merely increasing search depth remains an unvalidated remedy.

The `diagnostic-plain-N.txt` files were produced with `policy-bench replay --input .../diagnostic-plain.jsonl --duel N --which game`. The accompanying `diagnostic-protocol-N.txt` files replay the same recorded responses through `Core::replay`, printing each parsed engine message and its zero-based response number. They reveal both players' hidden cards for offline diagnosis; they are not policy inputs.
