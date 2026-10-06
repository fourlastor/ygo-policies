# Six Samurai, Gravekeeper, and Machina

These were the three weakest remaining policies in the tuning queue: Legendary Six Samurai (#12, 60.0%), Gravekeeper's Tomb (#6, 65.8%), and Machina Gadgets (#3, 69.0%). Baseline: `de2bf4d57a91469fc74cd1340ccab0f6352ec14a`, after Lightsworn and Fusion Heroes. Work is on `codex/samurai-gravekeeper-machina-policies`. Decks, vendor pointers, and shared policy code are unchanged.

## Held-out validation

After choosing the rules, each version played 512 fresh games against every other deck, seed `43780000`: 15,360 paired games per policy. Both versions faced the same baseline opponents, shuffles, seats, and policy randomness. Draws count as half a win. These results were not used for further tuning.

| Pilot | Before | After | Gain, pp [paired 95% CI] |
|---|---:|---:|---:|
| Legendary Six Samurai | 59.51% | 70.83% | +11.32 [+10.70, +11.95] |
| Gravekeeper's Tomb | 65.41% | 67.25% | +1.83 [+1.41, +2.25] |
| Machina Gadgets | 70.36% | 78.40% | +8.03 [+7.38, +8.69] |

Matchup movements (512 pairs per opponent):

- Legendary Six Samurai: 30 improved, 0 tied, 0 declined.
- Gravekeeper's Tomb: 20 improved, 4 tied, 6 declined.
- Machina Gadgets: 29 improved, 0 tied, 1 declined.

Negative matchup point estimates, retained without additional tuning:

- Gravekeeper's Tomb vs `arcana`: 96.48% → 96.29%, -0.20 pp [-1.58, +1.19].
- Gravekeeper's Tomb vs `burn`: 86.52% → 85.35%, -1.17 pp [-3.59, +1.25].
- Gravekeeper's Tomb vs `machina`: 49.61% → 47.66%, -1.95 pp [-4.71, +0.80].
- Gravekeeper's Tomb vs `quickdraw-plant`: 77.93% → 77.54%, -0.39 pp [-2.75, +1.97].
- Gravekeeper's Tomb vs `rock-block`: 67.19% → 66.21%, -0.98 pp [-3.82, +1.86].
- Gravekeeper's Tomb vs `verdict`: 18.75% → 18.16%, -0.59 pp [-2.98, +1.81].
- Machina Gadgets vs `countdown`: 0.98% → 0.78%, -0.20 pp [-1.21, +0.82].

Intervals are paired normal approximations; individual matchup intervals are exploratory and not adjusted for multiple comparisons. No validation games failed, and no candidate game reached the decision limit. One baseline Gravekeeper game against Infernity reached the limit and remains scored as a draw; the candidate finished it normally and won.

## Rules retained

Six Samurai:

- Search for Kagemusha when Kageki or a Level 3 Samurai can use the missing Tuner. After spending the Normal Summon, prefer free summons such as Kizan, Grandmaster, or the Shinai/Mizuho pair according to the visible hand and field.
- Have Kageki summon the missing Kagemusha instead of another attacker; avoid adding a redundant Tuner.
- Activate additional Six Samurai United copies before summoning.
- Normal Summon Kagemusha or Shien's Squire when they connect the current field to an available Synchro.
- Use legal Asceticism activations in Main Phase 1 to extend the board.
- Revive Kagemusha with Return of the Six Samurai when it enables a Synchro, or revive Kagemusha plus a Level 3 Samurai with Double-Edged Sword Technique when Shi En is available. Keep the existing 4,000-LP reserve for the latter.

Gravekeeper:

- Recognize Visionary's one-Gravekeeper Tribute procedure, and choose it when both summon procedures are offered. The generic two-Tribute valuation was suppressing useful summons.
- Use the Normal Summon before Royal Tribute discards the last monster in our hand.
- Do not cycle Pharaoh's Treasure into Necrovalley while its Graveyard recovery is blocked; Chief still enables it by exempting our Graveyard.
- Filter Descendant's removal targets through the existing public effect-protection knowledge.

Machina:

- Special Summon Cyber Dragon before filling the empty field with another monster.
- Let the normal summon sequence resolve Gadget/Gearframe searches before paying Fortress's discard cost.
- Prefer discarding Fortress itself: it can contribute to the summon cost and return immediately, preserving the smaller Machines and their searches.
- Use Chimeratech Fortress Dragon to consume opposing face-up Machines; retain multiple allied attackers when contact Fusion would consume only our field.
- Use Card Trooper's boost when a Battle Phase is available and more than six cards remain in the Deck.

All changes use permitted observations, public card data, and legal choices. They do not access opposing hidden identities, deck order, engine snapshots, or future random outcomes.

## Development measurements

Each screen used seed `42670000`, 256 games against each of the 13 reference policies: 3,328 paired games per tested policy. Screens are exploratory. A dash means that policy was not tested in that screen.

| Screen | Six Samurai rule / gain | Gravekeeper rule / gain | Machina rule / gain |
|---|---|---|---|
| screen01 | `kageki-tuner` / +1.95 pp | `assailant-flip` / +0.00 pp | `cyber-first` / +2.16 pp |
| screen02 | `search-extender` / +1.20 pp | `spy-first` / +0.06 pp | `gearframe-first` / +2.28 pp |
| screen03 | `united-all` / +0.84 pp | `spy-assailant` / -0.09 pp | `discard-fortress` / +4.75 pp |
| screen04 | `gateway-pool` / +0.09 pp | `tribute-aggressive` / -0.42 pp | `chimeratech-opponent` / +0.63 pp |
| screen05 | `tuner-normal` / +1.86 pp | `descendant-cheap` / -1.74 pp | `trooper` / +0.48 pp |
| screen06 | `shi-en` / +0.15 pp | `cannon-lethal` / +0.12 pp | `fortress-late` / +4.07 pp |
| screen07 | `united-one` / +0.24 pp | `raigeki-proactive` / -0.09 pp | `gearframe-score` / +0.09 pp |
| screen08 | `asceticism` / +1.20 pp | `assailant-position` / +0.03 pp | — |
| screen09 | `search-combo` / +5.41 pp | `assailant-safe` / +0.03 pp | `normal-before-fortress` / +3.71 pp |
| screen10 | `free-first` / -0.36 pp | `compulsory` / +0.18 pp | `no-duality-lock` / -0.33 pp |
| screen11 | — | `descendant-duplicate` / +0.12 pp | `trooper-guard` / +0.51 pp |
| screen12 | — | `descendant-reaches` / +0.12 pp | — |
| screen13 | — | `assailant-reaches` / +0.00 pp | — |
| screen14 | `revive-tuner` / +1.14 pp | — | — |
| screen15 | — | `tribute-after-summon` / +0.36 pp | — |
| screen16 | — | `visionary` / +2.24 pp | — |
| screen17 | — | `boosted-normal` / +0.48 pp | — |
| screen18 | — | `spy-empty` / +0.12 pp | — |
| screen19 | — | `treasure-live` / +0.54 pp | — |
| screen20 | — | `visionary+treasure-live` / +2.67 pp | — |

The combined screen gave **Legendary Six Samurai +12.56 pp**, **Gravekeeper's Tomb +3.03 pp**, **Machina Gadgets +10.37 pp**. The final development run expanded to 1,024 games per reference opponent (13,312 pairs per policy):

| Pilot | Before | After | Gain, pp [paired 95% CI] |
|---|---:|---:|---:|
| Legendary Six Samurai | 56.14% | 68.25% | +12.11 [+11.41, +12.80] |
| Gravekeeper's Tomb | 62.38% | 65.13% | +2.75 [+2.27, +3.23] |
| Machina Gadgets | 69.00% | 79.90% | +10.90 [+10.14, +11.66] |

All 9,984 selected-screen scores and decision digests reproduced in the larger development run after source cleanup.

Neutral or harmful candidates were omitted, including broad Spy/Assailant priorities, early one-counter United draws, Gateway counter pooling, always prioritizing Shi En, moving Samurai free summons earlier, a stricter Descendant sacrifice threshold, looser Royal Tribute costs, Assailant trigger changes, duplicate-Descendant sacrifices, proactive Raigeki Break/Compulsory, boosted small-Gravekeeper summon scores, changing Gearframe's general score, and stricter Pot of Duality gating. The more complete Samurai search rule superseded the narrower Kizan-only rule; the normal Fortress timing superseded the explicit Gearframe-first rule.

The isolated Visionary screen had one decision-limit draw against Infernity (seed `871220118`, Gravekeeper first). It reproduced exactly at 4,096 decisions / 215 turns. The trace shows Pharaoh's Treasure repeatedly activated under Necrovalley without Chief. Gating that activation finished this same case in 1,206 decisions / 63 turns (a loss, rather than a limit draw). The Visionary-plus-Treasure screen improved by 2.67 pp with zero limits. This was resolved before held-out validation.

## Search headroom

Both searches used seed `41560000`, eight games per reference opponent, 8/32/96-world stages, strict mode, and foresight disabled. None of these three pilots belongs to the reference pool, so all opponents remain identical baseline policies in both searches.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Legendary Six Samurai | before | 104 | 65.4% | 82.7% | 17.3 pp |
| Legendary Six Samurai | after | 104 | 73.1% | 90.4% | 17.3 pp |
| Gravekeeper's Tomb | before | 104 | 56.7% | 76.0% | 19.2 pp |
| Gravekeeper's Tomb | after | 104 | 60.6% | 77.9% | 17.3 pp |
| Machina Gadgets | before | 104 | 70.2% | 89.4% | 19.2 pp |
| Machina Gadgets | after | 104 | 81.7% | 92.3% | 10.6 pp |

An ordinary paired comparison reproduced all 312 before/after standalone search scores and decision digests. Searches had no failed playouts or decision-limit draws.

Search repeatedly chose Samurai Tuners, Asceticism, and the temporary revival traps; Machina search often delayed Fortress for a search-producing Normal Summon, summoned Cyber Dragon earlier, or used Card Trooper. Gravekeeper search supplied Visionary summons and removal-timing candidates. Card-selection priorities also came from policy and pinned-script inspection: search does not explore card-selection alternatives.

These small samples estimate headroom, not hard ceilings. Search has privileged opposing-deck composition and engine random-state access. Its one-step continuations depend on the pilot, so the searched result is not a monotonic upper bound as the pilot improves.

## Canonical ranking and checks

The full 31-policy round robin used seed `730000`, 256 games per unordered pair: **119,040 games**, without failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Legendary Six Samurai | #12 → #4 | 60.0% → 70.4% |
| Gravekeeper's Tomb | #6 → #6 | 65.8% → 67.1% |
| Machina Gadgets | #3 → #3 | 69.0% → 77.0% |

All **96,768 games involving none of these three policies** reproduced scores and decision digests from the preceding Lightsworn/Heroes ranking. The [tier list](../DECK-TIER-LIST.md) reproduced byte-for-byte from saved results. Tournament rates differ from the paired held-out rates because the seeds and opponent versions differ.

The workspace passed **169 tests**, including 12 new regressions for Samurai searches, Tuners, revival and United sequencing; Visionary and Royal Tribute; Treasure's Necrovalley/Chief interaction; and Machina's summon/discard order, contact Fusion and mill reserve.

- [Compact evidence](samurai-gravekeeper-machina-2026-10-07.summary.json)
- [Run metadata and hashes](samurai-gravekeeper-machina-2026-10-07.metadata.json)
- [Ranking summary](round-robin-2026-10-07-samurai-gravekeeper-machina.summary.json)
- [Ranking metadata](round-robin-2026-10-07-samurai-gravekeeper-machina.metadata.json)

Raw JSONL, search traces, and frozen binaries remain outside Git under `/tmp/ygo-samurai-gravekeeper-machina.yn4qne0f`. Their hashes and exact arguments are recorded.
