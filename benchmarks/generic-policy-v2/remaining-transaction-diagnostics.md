# Remaining transaction diagnostics

Read-only audit, 2026-10-14. Source references were checked in the working tree at HEAD `0a3c26cd1b04c05a9b9403277d618f69103e7fc4`; concurrent uncommitted work was present. These are proposals, not implemented changes or measured improvements. **The action evidence comes from old v8 traces, not a replay of the current v25/v26 candidate.** Current source still lacks the specific consumers described below, but changed earlier decisions may prevent the old states from recurring.

All row and trace indices below are zero-based. Only entries with `observation.me == row.seat` are attributed to Generic. Compressed files retain the original logical JSONL rows. No builds, tests, matches, or source edits were performed for this audit.

## 1. Soul Exchange needs a complete tribute transaction

Observed in `pilot-v8-traces.jsonl.gz`:

| Row / seed / seat | Exact observed sequence |
|---|---|
| 9 / `2004524038` / 0 | Turn 3: trace **42** sets Dupe Frog, **46** activates Soul Exchange with `summon_used=true`, **48** targets an opposing facedown monster, **53** ends without releasing it. Mobius remains in hand. |
| 11 / `2004524037` / 1 | Turn 2: trace **19** sets Treeborn Frog, **23** activates Soul Exchange with `summon_used=true`, **25** targets opposing Mobius, **30** sets Mystical Space Typhoon, **34** ends without releasing Mobius. Raiza remains in hand. |

`vendor/CardScripts/official/c68005187.lua:18-32` pays the battle-phase restriction on activation; lines 40-47 grant the target an extra-release permission only until the end of the turn. The spell itself removes nothing.

Current code:

- `crates/ygo-policies/src/generic/knowledge.rs:360-364` describes `SkipBattlePhaseThisTurn` and `UseTargetAsTributeThisTurn`.
- **The fact is already consumed narrowly** in `generic/transactions.rs:592-603`: Worm King's cost guard preserves a possible opposing Worm tribute after a known permission effect. It is not wholly unused.
- There is no corresponding activation/setup/main-tribute consumer in `generic.rs:170-207`. `agent.rs:554-575` tries normal summons before Generic's late unknown activation fallback (`generic.rs:449-456`).
- An ordinary `NormalSummon` choice (`model.rs:328`) does **not** expose its actual tribute cost or procedure. Default `tactics.rs:44-52` counts and prices only our monsters; `own.len() < tributes` rejects a candidate if the strategy falls through. Generic's existing legal-summon override (`generic.rs:463-479`) bypasses that shortage check in its covered case, but does not identify the actual permitted enemy material. With enough own monsters, the fallback can still price the wrong costs. Early activation alone is therefore not a complete fix.

Proposed scope: a source-specific setup rule tied to a verified same-turn release payoff, followed through the native normal-summon and release prompts. Preserve effect-tribute and extra-normal-summon exceptions; do not blanket-veto solely because `summon_used` is true. Do not assume a printed high-level card necessarily uses a standard tribute procedure. Also check the destination's mandatory arrival effect: consuming the opponent's sole card before Raiza arrives can force an undesirable own target, so the second trace does not prove that early Exchange into Raiza would have been good.

First fixture: both seats, Soul Exchange + Mobius + a low-level monster in hand, no own monster, an opposing releasable monster and backrow. Exercise the **actual native** sequence: Exchange before the ordinary summon, newly offered legal Mobius summon, and opposing release material. Include an own-expensive-monster variant to expose incorrect fallback cost pricing, plus spent-normal/no-release-payoff and extra-release-effect exceptions. Replaying these two old seeds with the current library is still required before claiming the observed failure persists.

## 2. Optional deck spending lacks a mandatory-mill budget

Observed in `pilot-v8-weak-traces.jsonl.gz`, row **18**, seed **859105550**, seat **1**:

- Turn 20, trace **560**: Generic activates Charge of the Light Brigade with **five cards** in its deck and face-up Lyla, Aurkus and Lumina. Plaguespreader activation is an explicitly offered alternative. Charge mills three; traces **564-566** show **two cards** remaining and no search selection or added card in this resolution.
- Trace **566** revives Plaguespreader and returns a hand card to the deck. At **574**, with three cards remaining, Generic activates Glow-Up Bulb while multiple Extra summons are already offered. The deck falls to two.
- **583-586** Synchro Summon Scrap Dragon using Bulb, Lumina and Lyla. After the following interaction, **605** ends with Aurkus and **two deck cards**. Its mandatory effect at **607-610** mills both. The opponent draws its last card at **612**, then Generic fails its next draw on turn **22** (final result reason `2`).

This establishes the actual spending/deadline sequence, **not** that skipping Charge or choosing a particular Extra summon necessarily wins. At trace 560, the existing board already threatens more mandatory milling than the deck can sustain; a useful rule must value removal of those obligations as well as refusing optional spending.

Native facts:

- `vendor/CardScripts/official/c94886282.lua:16-33`: Charge sends three as cost, then searches if a matching card remains. Categories alone omit the cost.
- `c691925.lua:23-37`: Solar Recharge draws two and mills two; its deck requirement does not reserve the next draw.
- `c67441435.lua:16-28`: Bulb sends one deck card as part of its summon operation.
- `c7183277.lua:14-30`: face-up Aurkus must mill two in its controller's End Phase; `c95503687.lua:52-60` and `c22624373.lua:58-66` specify Lumina's and Lyla's three each.

Current `generic.rs:311-353` prioritizes unknown search/draw or graveyard access without these quantities or a reserve for mandatory end-phase effects. No corresponding source-specific budget is present in `generic/life.rs`. Proposed scope: compositional per-card deck-consumption/mandatory-event facts, conditioned on visible active sources and public suppression, used to protect the next draw and to prefer actual offered continuations that remove the obligation. Do not copy the specialist's entire flow or impose a blanket low-deck prohibition when a concrete winning line exists.

First fixture: both seats, own face-up Aurkus, a four-card deck, legal Charge, no immediate winning continuation. Verify the pilot preserves the next draw; retain Charge with sufficient reserve or disabled mandatory milling. A separate native continuation fixture should show that consuming the miller changes the budget. The old seed needs a current-library trace before a strength claim.

## Search-budget evidence boundary

No clean modern multi-deck **32 versus 128 iterations** comparison was found in the saved metadata audit. `fast32-v16/report.md` records one Demise development comparison: **35/64 at 32 versus 32/64 at 128**, 12 gained and nine lost wins, paired interval −9.4 to +18.8 percentage points. This does not establish equivalence. The v16 multi-deck sentinels compare libraries at fixed 128 iterations. Historical v6 128-to-512 runs also changed worlds, action limits, nodes, validation settings and work limits.

Smallest proposed fresh screen: Heroes, Gishki, Gem-Knight and Machina; **16 paired games per deck per arm**, eight per seat, using a new predeclared seed. Freeze current host/library/default weights and all settings except iterations 32/128. Report per-deck discordant outcomes, work completion, quality and latency; pooled strength is exploratory. A null result from this small screen would remain inconclusive. No run is authorized or launched by this document.
