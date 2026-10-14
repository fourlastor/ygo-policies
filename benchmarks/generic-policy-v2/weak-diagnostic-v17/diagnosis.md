# Selected replay findings

These are post-hoc observations from the exact v17 replay, not new strength evidence. Row and trace indices below are **zero-based** in the preserved logical `search32.jsonl` stream; compression leaves them unchanged. The Psychic review came from `coverage_audit`, the Exodia review from `effect_metadata`; the cited decision/state entries were also checked during the artifact audit.

## Psychic Commander payment

**Row 18, seed 1467226049, candidate seat 0, turn 14.** Trace 373 activates Psychic Commander (21454943). Our face-up attack-position Commander has 1,400 ATK in monster zone 4 and is being attacked by the opponent's 1,600 ATK Destructotron (11232355) in monster zone 1; LP are 1,100/4,800. Trace 374 offers 100, 200, 300, 400 and 500, and chooses **100**.

Trace 375 accepts Brain Research Lab's replacement, so this activation does not immediately pay the 100 LP. At trace 378 the attacker has 1,500 ATK/300 DEF. Trace 384 shows our LP at 1,000 from battle damage; trace 386 shows Commander in graveyard slot 9. Thus the selected amount leaves Commander losing this recorded battle. A 300 reduction would put this attacker at 1,300 ATK, below Commander, **holding the remaining response context fixed**; no alternative duel continuation was simulated here.

The reviewer counted 11 candidate Commander activations and 11 numerical selections across the 16 Psychic games, all choosing 100. The specialist activated Commander twice, with one numerical prompt choosing 300. The frozen Generic policy had no Commander-specific announcement override and selected the first numerical option. The pinned native script `official/c21454943.lua` offers the affordable 100–500 amounts and applies the chosen reduction. This supports a specific payment-choice defect, not a claim that fixing it would change the match result.

## First Heart versus a missing Exodia piece

**Row 8, seed 1326585873, candidate seat 1, trace 108, turn 6 Main1.** Pot of Duality offers choice 0 Left Leg (44519536), choice 1 Heart of the Underdog (35762283), and choice 2 Charcoal Inpachi (13179332). Generic chooses **0**. It holds only Right Arm (one of five distinct pieces), no Heart in hand or spell/trap zone, and two ordinary Normal monsters in hand. Two persistent spell/trap occupants remain after Duality resolves, leaving installation space.

In frozen library `9bacb13`, `generic/alternate.rs` values this missing-piece selection at 55,000, while the first Heart's ordinary value is 7,000. Duality's pinned native script `official/c98645731.lua` adds one of the three revealed cards and shuffles the rest. Heart's `official/c35762283.lua` supports repeat Draw-phase draws after a Normal monster draw. The first own Heart appears in hand at trace 350, turn 18 Draw, and is activated at trace 358 in Main1. It triggers repeatedly on turn 20 (traces 394, 398, 401, 408, 416, 420, 423 and 426); a second Heart is activated at trace 437. The opponent wins on turn 21, reason 16.

This particular game recorded **zero searches and no deviations**, so the observed choice followed Generic. Only one of the 16 Exodia replays offered a first Heart and a missing piece simultaneously under these conditions. The late draw engine is observable; it is not established that the earlier choice caused the loss or that prioritizing Heart would improve aggregate results.
