# Crystal replay observations

Diagnostic replay of the frozen host `62614ae` / policy library `9bacb13`; this is **not independent strength evidence**. `replay-checks.json` confirms that all 16 plain and 16 search outcomes and record digests match the original roster run. The six selected regressions do not establish which rule caused an outcome. No additional games or tests were run for this report.

JSONL line numbers below are one-based; `game.trace` indices are zero-based. Generic decisions are filtered by `entry.observation.me == row.seat`. Where an alternative is claimed, it appears in that recorded decision's choices. A plain trace containing only its selected combination does not establish the complete original candidate set.

## 1. Full backrow leaves hand spells unusable

`search32.jsonl` line 7, seed **1109097477**, seat **1**:

- Trace **252**, turn 10 Main1: S/T slots 0–4 contain Ruby, Eagle, Cat, Ruby, Pegasus. The only own monster is Malefic Rainbow Dragon. Hand: Abundance, Blessing, Release, Promise, Tiger. The legal choices contain NormalSummon Tiger, ChangePosition Malefic, SetMonster Tiger, EnterBattle, EndTurn, ShuffleHand; **no spell activation** is offered. Search chooses EndTurn, reported gain +0.225.
- It chooses EndTurn again at **264** (+0.297) and **308** (+0.231). The previous evaluator counted hand spells as accessible resources plus their action priors without charging this zone constraint.
- Trace **216** previously skipped legal Abundance before the fifth stored Beast filled the zone. That skip is **not proven wrong**: the opponent's public Ruins, stored Beasts, and monster could negate the activation after its four-card cost.

This supports the currently isolated **generic full-S/T congestion valuation** change. It does not justify forcing Abundance or rejecting every fifth stored Beast; native exits, already-set spells, Hamon, and Ruins can change the value of a full backrow.

Complementary case: `search32.jsonl` line 11, seed **1109097476**, seat **0**, traces **130, 148, 166, 182, 195, 208, 226, 239, 253, 274, 288** (turns 5–25) repeatedly choose EndTurn over legal face-down Promise. Own board begins as Turtle in defense plus stored Ruby/Cat/Cat/Mammoth and Promise in the fifth slot. This board has an actual exit; a blanket full-backrow penalty alone cannot model its usefulness.

## 2. Ruby's special-summon payoff is absent from target value

`search32.jsonl` line 12, seed **1109097484**, seat **0**, turn 9:

- Trace **280** activates Promise. At **282**, all three recorded targets are own stored Tiger, Ruby, Ruby. The policy selects Tiger. There is one own monster, Pegasus, leaving four open monster zones before resolution. The summon resolves at **285–286** and battle begins at **289**.
- Ruby is not merely a 300 ATK recipient: its optional special-summon trigger can summon the remaining stored Beasts, limited by available monster zones. Selecting Ruby here can provide Tiger and the second Ruby as well as the selected Ruby. This is an unpriced continuation, not proof that spending all the stored resources is always best: own Ruins and future resource needs must also be considered.

The same legal-target issue appears in line 11, seed **1109097476**, trace **316**: Promise selects Mammoth over offered Ruby/Cat/Cat with two own monsters and four stored Beasts. **That actual Promise is subsequently negated**, so do not credit a counterfactual successful Ruby line as an observed improvement. Line 13, seed **1109097483**, trace **142**, similarly selects stored Pegasus over offered Ruby/Pegasus/Ruby.

Source: `vendor/CardScripts/official/c32710364.lua:13–22,40–67`. Generic `gain_value` in `crates/ygo-policies/src/generic.rs` prices SpecialSummon recipients by body/combat stats, without this companion payoff. A narrow future improvement can add the source-backed Ruby continuation value using actual stored companions and remaining zones, conditioned on public summon/attack restrictions and the resource value lost from storage. Test a legal Promise target fixture with unfamiliar surrounding cards, plus full-monster-zone, Malefic, and Ruins-threshold counterexamples.

Do **not** use line 13 trace **437** (Ruby's YesNo declined) as an unequivocal mistake: own Malefic prevents the other monsters attacking, and moving companions would lose Ruins thresholds.

## 3. Direct attack damage is overstated for Cat

`search32.jsonl` line 16, seed **1109097486**, seat **0**:

- Opponent Cat attacks at **346**, selects direct attack at **347**, with five opposing monsters remaining. The next LP change at **358** is **3350 → 2750**, i.e. **600** damage from 1200 ATK.
- Opponent Cat attacks again at **542**, direct choice **543**; at **555**, LP changes **1250 → 950**. Cat's 600 damage is further halved by Ruins to **300**.
- Independent same-mechanic example: line 11, seed **1109097476**, opponent Cat attack **445**, LP **8000 → 7400** at **457**.

Source: `vendor/CardScripts/official/c32933942.lua:13–24,41–45`: direct damage is halved when an opposing monster remains and the card has fewer than two direct-attack effects. An open-field direct attack is not halved. `crates/ygo-policies/src/knowledge/pool.rs` marks Cat `direct: true`; `battle_projection` in `crates/ygo-policies-bench/src/engine/evaluation.rs` currently credits full ATK for direct attacks.

A future exact damage fact should use projected remaining targets, retain full open-field damage, and compose with Ruins without conflating the two reductions. Additional direct-attack effects/negation need conservative treatment if not observable. No Generic-controlled Cat attack occurred in the six regressed search games; this demonstrates an exposure-model error, **not** an established cause of these regressions.

## 4. Crystal Release can buff the opponent

This is outside the six selected regressions, but is a concrete error in the same completed replay set.

`search32.jsonl` line 1, seed **1109097475**, seat **1**, turn 6:

- Trace **150** activates Release. At **152**, recorded legal targets include own Cat and opponent Tiger/Mammoth/Mammoth. The policy selects **opponent Tiger**, already at 2400 ATK. At **155**, the opponent Tiger is visibly **3200 ATK**; own LP is 500.
- Plain also equips opposing monsters: `plain.jsonl` line 1 seed7475 trace **85**, line 9 seed7481 trace **29**, line 15 seed7482 trace **57**. These plain records establish selected targets, not complete alternatives.
- A separate own-target ranking issue occurs in search line 11 seed7476 trace **325**: Release chooses defense-position Turtle over offered attack-position Tiger. Their printed defense/body values are not the incremental attack benefit of this equip. The later search changes Turtle to attack at **332**.

Source: `vendor/CardScripts/official/c10004783.lua:5–11` allows either player's Crystal Beast and adds 800 ATK. Generic's opponent `select` fallback returns the shared gain score, which uses opposing threat; the source's beneficial target polarity is absent. A narrow future fix can require a useful own recipient for exact Release activation/Equip prompts and rank the incremental battle benefit, leaving unusual deliberate enemy-equip lines to engine search. This is a card fact, not a deck rule.

## 5. Additional evaluator observations from counter_review

- `search32.jsonl` line 6, seed **1109097479**, seat 1, trace **126**: own Ruins with four stored Beasts but **zero own monsters**. The old interaction prior is still 0.25 although Ruins' Spell/Trap negate requires sending an own face-up Crystal Beast **monster** as cost. The actual No here is sensible: it saves the half-damage use from Ruby's 300 attack for the later Mammoth attack at **142**. Do not label that deviation a failure.
- Line 16 seed **1109097486**, trace **321** uses Ruins' five-stored summon; Ruby then releases the stored bank. At **340**, own board has five monsters, zero stored Beasts, and Ruins. The previous evaluator still gives Ruins access 0.35 and interaction 0.25, or 0.3575 after the applicable weights, despite losing its enabled thresholds.
- Ruins' actual source thresholds are in `vendor/CardScripts/official/c34487429.lua`: two stored for optional once-per-turn damage halving (**69–78**), three stored plus own monster cost for negate (**80–103**), four for draw (**106–118**), five for summon (**120–139**).

These Ruins, Ruby, Cat, and Release changes are preserved for separate experiments. At report creation, only the generic full-S/T congestion change is authorized for the next isolated candidate. The trace files were not modified or archived.
