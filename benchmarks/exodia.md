# Astra's Exodia: Heart of the Underdog replacement

The current list replaces the initial Hope for Escape stall build with **three Hearts of the Underdog, three Reloads, and 22 Normal Monsters**, including Exodia's four limbs. It keeps a defensive core and all five Exodia pieces. The user selected this faster build after reviewing its overall win-rate tradeoff.

The refreshed 32-policy tournament ranks it **#2**, at **79.39%** over 7,936 games. Independent held-out validation gives **78.48%** over 15,872 games, 95% Wilson interval [77.84%, 79.12%]. The tier remains **strong**. The [current ranking](../DECK-TIER-LIST.md) was regenerated.

## What changed in the same held-out matchups

512 games against each of the 31 other policies, alternating seats, seed `56250000`. These data were held out from deck and policy selection.

| Measure | Previous Hope build | Current Heart build |
|---|---:|---:|
| Overall win rate | 80.24% | 78.48% |
| Against Burn Princess | 15.62% | 49.61% |
| Against Countdown | 21.09% | 80.66% |
| Median winning duel turn | 31 | 16 |
| Mean winning duel turn | 31.16 | 17.78 |
| Wins completed by duel turn 8 | 65 | 1063 |

Duel turns count both players' turns. Speed statistics are conditional on winning; they do not compare identical sets of won games. The overall change is **-1.75 percentage points**, with a paired 95% interval [-2.52, -0.98]. This is a measured tradeoff, not an overall win-rate improvement. Both the deck and its policy changed. Neither version had a failure or a decision-limit draw.

Of the new build's 12,457 wins, **12,336 win by Exodia**, 81 by opponent deck-out, and 40 by Life Points (including battle damage when attackers hit defensive monsters).

## The 40-card list

- One each of the five Exodia pieces.
- Three each: Battle Footballer, Charcoal Inpachi, Renge, Gatekeeper of Dark World, Gem-Knight Sapphire, Skull Dog Marron, Giant Soldier of Stone.
- Three each: Heart of the Underdog, Reload, Pot of Duality, Messenger of Peace, Skill Drain.
- One each: Level Limit - Area B and Gravity Bind.
- Empty Extra and Side Decks.

The 18 defensive Normal Monsters have 2,000–2,100 DEF. Messenger, Level Limit and Gravity Bind buy time; Skill Drain protects against monster effects. Every card and copy count passes the local WC2011 pool and limit list.

## Pilot decisions and the loop-guard fix

Activate Heart early and use Duality to find the engine, while always taking the winning fifth piece. Set defensive Normal Monsters and preserve every Exodia piece during discards. Keep one lasting attack lock instead of filling every Spell/Trap zone with redundant locks. Reserve room for Skill Drain and a zone in which Reload or Duality can be played.

Reload waits for the player's Draw Phase while Heart is active, and waits until the current draw chain resolves. Without a Heart, it can cycle a sufficiently large hand during the Main Phase to find the engine. Multiple Hearts may chain above each other, but pending draws must fit in the remaining Deck.

A trace audit exposed a shared guard that refused a fourth use of the same action in one turn. That truncated legal repeated Heart triggers, especially with multiple copies. A regression test reproduced the failure before the fix. The shared guard now has an opt-in strategy hook; only this pilot's engine-offered Heart triggers during its own Draw Phase use it. Every other policy retains the previous guard.

The audited duels contain three simultaneous Hearts and up to **32 Heart activations in one Draw Phase**. All five remaining declined trigger opportunities in the 64-game corrected audit were cases where pending draws already consumed the remaining Deck.

The policy sees its own hand, public board/history, pile counts and legal selection prompts. It never reads hidden cards or search state.

## Development screens, including Upstart and Jar

24 distinct deck configurations were tested across four policy stages, 3,968 games per completed screen (128 per opponent), seed `56140000`. The 9 stage-01 screens predate the guard fix and are diagnostics, not final evidence against those decks. There are 32 completed screens after the fix; all finish without failures or limits. The [compact summary](exodia-underdog-2026-10-07.summary.json) records all 41 runs.

Stages 02–04 progressively change Spell/Trap space management. Compare variants within a stage to isolate the deck swap; a comparison across stages changes both the list and its pilot.

| Configuration | Policy stage | Overall | Burn | Countdown | Median winning turn |
|---|---:|---:|---:|---:|---:|
| Normal-heavy, no support | 02 | 45.56% | 61.72% | 80.47% | 8 |
| Normal-heavy + Reload | 02 | 56.17% | 67.97% | 87.50% | 6 |
| Reload + Duality | 02 | 55.77% | 73.44% | 92.19% | 7 |
| Light locks | 02 | 65.17% | 60.94% | 87.50% | 10 |
| Light locks + Upstart | 02 | 63.66% | 52.34% | 89.84% | 11 |
| Light locks + Jar | 02 | 65.93% | 61.72% | 89.84% | 10 |
| Light locks + both | 02 | 64.64% | 52.34% | 92.19% | 11 |
| Defense + two Reloads | 02 | 77.75% | 44.53% | 76.56% | 18 |
| Defense + Upstart | 02 | 76.79% | 42.97% | 67.19% | 19 |
| Defense + Jar | 02 | 76.99% | 41.41% | 76.56% | 19 |
| Defense + both | 02 | 77.14% | 40.62% | 68.75% | 20 |
| Selected faster build | 04 | 78.43% | 44.53% | 85.16% | 16 |
| Selected build + three Steelcages | 04 | 79.56% | 39.84% | 71.88% | 20 |

Jar can restart Heart during the Draw Phase: a 16-game native trace audit observed 28 Draw Phase Jar activations and 12 subsequent restarted Heart sequences. Upstart helps find the engine in the Main Phase. Both also replace Normal Monsters in the list and can interrupt a Heart sequence when drawn. Jar adds a set-turn delay and competes for Spell/Trap space. Their measured effect depends on the rest of the list; neither earned a slot in the selected build. They were tested, not dismissed as incompatible.

Three additional Steelcages raised the development aggregate slightly, but slowed the deck and reduced its Countdown advantage. The selected list preserves the faster result the user requested. Other screens included Hand Destruction, Gold Sarcophagus, extra defenses and fewer Dualities; their complete results are in the summary.

## Search headroom

A separate 104-game search against the 13 reference decks, seed `56360000`, 8/32/96 worlds, strict mode, no foresight, measured **69.23% → 77.88%**, a **8.65-point gap**. It used 124,664 playouts, with zero failed playouts and zero decision-limit draws, and completed in 252 seconds on six workers.

Search frequently changes Reload timing and the timing or placement of defensive cards. These are leads for later testing, not proven standalone improvements. This small sample estimates remaining headroom; it is not a strategy ceiling or a guaranteed policy gain. Search has privileged access; the exported policy does not. All 104 baseline outcomes and response digests reproduce on the final source.

## Held-out matchups

| Opponent | Hope | Heart |
|---|---:|---:|
| burn | 15.62% | 49.61% |
| draconic-might | 76.37% | 50.59% |
| monarch | 70.12% | 56.84% |
| morphtronic | 61.13% | 61.13% |
| six-samurai | 74.22% | 66.21% |
| dragunity | 75.59% | 67.19% |
| x-saber | 78.32% | 70.51% |
| gishki | 79.69% | 70.70% |
| tele-dad | 83.40% | 71.48% |
| gravekeeper | 58.59% | 71.88% |
| lightsworn | 89.45% | 74.22% |
| watt | 82.03% | 74.22% |
| harpie | 91.21% | 75.20% |
| gladiator | 80.66% | 75.39% |
| machina | 93.95% | 75.98% |
| spellcaster | 66.21% | 78.12% |
| quickdraw-plant | 92.97% | 79.69% |
| countdown | 21.09% | 80.66% |
| blackwing | 84.96% | 81.64% |
| toon | 94.34% | 81.64% |
| karakuri | 87.70% | 82.81% |
| infernity | 91.60% | 88.09% |
| heroes | 97.46% | 89.45% |
| verdict | 94.14% | 90.23% |
| fortune-lady | 68.75% | 90.43% |
| pyramid | 97.46% | 92.19% |
| rock-block | 90.23% | 94.34% |
| ojama | 94.14% | 96.88% |
| crystal | 98.24% | 97.46% |
| arcana | 99.41% | 98.83% |
| destiny-hero | 98.24% | 99.41% |

## Validation and reproduction

- **179 workspace tests pass**, including the native long-duel arena regression from the initial Exodia work.
- Final cleanup reproduces all 3,968 selected-development outcomes and response digests.
- All 15,872 held-out games reproduce on the final source.
- The full 126,976-game tournament has zero failures and zero decision-limit draws.
- All **119,040 games between the other 31 policies** have unchanged outcomes and response digests compared with the preceding tournament. The shared hook changes no existing opponent's behavior.
- The engine is unchanged from the initial Exodia validation; that work's arena-capacity fix and native fixture remain included. See the [initial Hope report](exodia-hope.md#native-engine-capacity-and-validation).

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench matchup --policies exodia --opponents all \
  --games 512 --seed 56250000 --workers 4 --output /tmp/exodia-heart-heldout.jsonl
target/release/policy-bench search --policies exodia --opponents existing \
  --games 8 --seed 56360000 --worlds 8 --confirm 32 --final 96 \
  --strict true --foresight false --workers 6 --output /tmp/exodia-heart-search.jsonl
target/release/policy-bench round-robin --policies all --games 256 \
  --seed 730000 --workers 8 --output /tmp/exodia-heart-ranking.jsonl --markdown DECK-TIER-LIST.md
YGO_CARDS_CDB="$PWD/vendor/BabelCdb/cards.cdb" cargo test --workspace
```

Compact [metadata](exodia-underdog-2026-10-07.metadata.json) records source, library and vendor fingerprints. Raw games, diagnostic traces, binary libraries and discarded prototypes stay outside the repository. The earlier Hope investigation and online alternatives remain documented in [exodia-hope.md](exodia-hope.md).

## Display-name update

Renamed to **Astra's Exodia** on 2026-10-08. The policy ID remains `exodia`, and the card list and decision rules are unchanged. The original run metadata retains the measured name and fingerprints; the metadata's `rename` entry records the new name and source hashes. All nine deck/policy tests pass after the rename, and a 496-game check across all 31 opponents reproduces the pre-rename outcomes and response digests exactly.

## Rules checked

Official card text: [Heart of the Underdog](https://www.db.yugioh-card.com/yugiohdb/card_search.action?cid=5837&ope=2&request_locale=en), [Upstart Goblin](https://www.db.yugioh-card.com/yugiohdb/card_search.action?cid=4895&ope=2&request_locale=en), [Jar of Greed](https://www.db.yugioh-card.com/yugiohdb/card_search.action?cid=5210&ope=2&request_locale=en). The multiple-Heart and Draw Phase restart interactions were also verified in the pinned native engine and card scripts. Deck legality uses `data/wc2011.lflist.conf`, not the present-day tournament ban list.
