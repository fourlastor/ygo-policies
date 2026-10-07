# Initial Hope for Escape build: construction and validation

This records the initial build. The current Heart of the Underdog replacement is documented in [exodia.md](exodia.md).

Policy `exodia`, a 40-card WC2011 list. The final full tournament places it **#2 of 32**, with **80.51%** across 7,936 games and a **strong** reference-pool rating.

Independent held-out validation: **80.93%** across 15,872 games (512 per opponent, alternating seats), 95% Wilson interval [80.31%, 81.53%]. No failures or decision-limit draws. Of 12,845 wins, 12,579 used Exodia's actual victory condition; the other 266 were opponent deck-outs.

## How the deck works

Gift Card and Upstart Goblin increase the opponent's Life Points, enabling larger Hope for Escape draws. Messenger of Peace, Level Limit - Area B, Gravity Bind, Steelcage and Swords stop attacks. Skill Drain protects these locks against monster effects. Cyber Valley draws while ending a Battle Phase; Shining Angel recruits it. Battle Fader and Swift Scarecrow cover gaps.

The policy keeps all five pieces in hand, never Normal Summons or Sets them, and preserves them during End Phase discards. Duality prioritizes support early and the winning fifth piece immediately. Hope waits for Gift Card to resolve, and its expected draw must fit in the remaining Deck. Protection reuses Countdown's existing stateful cover/negation handling; neither that policy nor any other opponent policy changed.

Policy decisions see only their own hand, public field/history, pile sizes, and cards revealed by legal selection prompts. No search state or hidden-card inspection enters the policy.

## Final list

- 1 each of the five Exodia pieces.
- 3 each: Cyber Valley, Shining Angel, Battle Fader; 2 Swift Scarecrow.
- 3 each: Upstart Goblin, Pot of Duality, Messenger of Peace, Nightmare's Steelcage.
- 1 each: Level Limit - Area B, Swords of Revealing Light.
- 3 each: Hope for Escape, Gift Card, Skill Drain; 1 Gravity Bind.
- Empty Extra and Side Decks.

All cards and copy counts pass the repository's WC2011 pool and limit list.

## Development screens

Each completed screen used 128 games per current opponent, 3,968 games total, seed `54810000`. These are selection data, not the held-out score.

| Candidate | Full-pool win rate |
|---|---:|
| screen-01 | 30.42% |
| screen-03-trap-wall | 40.45% |
| screen-03-lock-hope | 80.24% |
| screen-04-gold-gift | 80.02% |
| screen-04-gold-cage | 76.76% |
| screen-04-gold-angel | 73.24% |
| screen-05-library-dragon | 26.44% |
| screen-05-library-rejuv | 18.52% |
| screen-05-library-bamboo | 21.55% |
| screen-06-trader | 79.23% |
| screen-06-mixed | 76.84% |
| screen-06-searcher | 79.99% |
| screen-06-recovery | 79.06% |

The selected list is `screen-03-lock-hope`. The initial draw-trap deck lacked lasting defense. Adding locks and Skill Drain produced the large gain; replacing those defenses with more searches reduced consistency. The Library implementations are prototypes, not strategy ceilings. Cleanup reproduced all 3,968 selected-development outcomes and response digests exactly.

An additional Wall of Revealing Light / draw variant failed to finish because of the same arena capacity issue described below; its incomplete run was excluded. Intermediate implementation smoke runs are not selection evidence.

## Held-out matchups

| Opponent | Win rate |
|---|---:|
| burn | 16.21% |
| countdown | 22.46% |
| gravekeeper | 62.70% |
| fortune-lady | 65.82% |
| morphtronic | 67.97% |
| spellcaster | 71.88% |
| dragunity | 75.59% |
| gishki | 75.98% |
| monarch | 77.54% |
| watt | 77.54% |
| gladiator | 78.71% |
| six-samurai | 78.71% |
| tele-dad | 79.88% |
| x-saber | 80.08% |
| draconic-might | 81.45% |
| blackwing | 84.96% |
| rock-block | 87.70% |
| lightsworn | 88.09% |
| karakuri | 90.43% |
| quickdraw-plant | 91.80% |
| infernity | 92.97% |
| ojama | 93.36% |
| harpie | 93.55% |
| machina | 94.14% |
| verdict | 94.53% |
| toon | 95.51% |
| pyramid | 95.70% |
| destiny-hero | 97.27% |
| heroes | 98.44% |
| crystal | 98.83% |
| arcana | 99.02% |

Burn bypasses the battle locks and punishes the LP payments. Countdown generally completes its clock before this list assembles Exodia. The strong overall result is against this roster, without side-deck adaptation.

## Search headroom

104 games against the 13 reference decks, seed `56030000`, 8/32/96 worlds, strict mode, no foresight: pilot **76.92%**, search **86.54%**, gap **9.62 percentage points**. There were no failed playouts or decision-limit draws; all 104 baseline results and response digests were independently reproduced on the final engine.

This is a small search sample and an estimate of remaining headroom, not an upper bound or a guaranteed policy improvement. Search has privileged access; the exported policy does not.

## Native engine capacity and validation

The first held-out run aborted in the Exodia/Tele-DAD duel at actual seed `1311823037` (CLI base seed `55920034`, two games). GDB stopped on `std::bad_alloc` in `duel_arena::allocate`, inside Lua/Synchro-material checks. The 256 MiB monotonic arena was exhausted.

The fork now reserves 1 GiB per duel instead of 256 MiB. This changes capacity, not rules or snapshot contents: snapshots still copy only used bytes. The reproduced duel uses 272,572,276 bytes (about 260 MiB) by its end, completes after 932 decisions on turn 43, and wins by Exodia. The exact dealt cards and responses are a permanent native replay regression fixture. The allocator remains monotonic and frees the arena when the duel is destroyed.

The post-fix held-out run completed all 15,872 games; its 13,380 previously completed games reproduce exactly. Both complete 126,976-game tournaments, before and after the capacity change, have identical outcomes and response digests. All 119,040 games between the prior 31 policies also match the preceding canonical tournament exactly. 176 workspace tests pass, including the native regression.

## Reproduction

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench matchup --policies exodia --opponents all \
  --games 512 --seed 55920000 --workers 6 --output /tmp/exodia-heldout.jsonl
target/release/policy-bench search --policies exodia --opponents existing \
  --games 8 --seed 56030000 --worlds 8 --confirm 32 --final 96 \
  --strict true --foresight false --workers 6 --output /tmp/exodia-search.jsonl
target/release/policy-bench round-robin --policies all --games 256 \
  --seed 730000 --workers 10 --output /tmp/exodia-ranking.jsonl --markdown DECK-TIER-LIST.md
YGO_CARDS_CDB="$PWD/vendor/BabelCdb/cards.cdb" cargo test --workspace
```

Compact results: [summary](exodia-2026-10-07.summary.json), [metadata](exodia-2026-10-07.metadata.json). Large raw histories and discarded prototypes remain outside the repository.

## Strategies checked online

- **Hope for Escape stall.** [Winston's 2012 Worlds list, transcribed by Road of the King](https://roadoftheking.wordpress.com/2012/08/10/yu-gi-oh-world-championship-2012-4/) uses draw traps, Gift Card, battle protection, and three One Day of Peace. The transcription links Winston's own video, which was unavailable to the browser. One Day of Peace is absent from this repository's WC2011 pool, so this was a strategic reference, not a legal list copied unchanged. [Konami's Hope card text](https://www.db.yugioh-card.com/yugiohdb/card_search.action?cid=9354&ope=2) confirms the Life Point differential and cost.
- **Library/Dragon draw.** A [WC2011 player's own deck and explanation](https://gamefaqs.gamespot.com/boards/612092-yu-gi-oh-5ds-world-championship-2011-over-the-nexus/58658643) combines Library, White Stone, Blue-Eyes, Consonance, Trade-In, Toon Tables, Dragon Ravine, and Super Rejuvenation. Three independently implemented prototypes tested the Dragon, Rejuvenation, and Bamboo/Citadel engines. They are bounded pilot experiments, not optimized ceilings for these strategies.
- **Bamboo/Citadel Library.** A [creator's Library deck on Neuron](https://www.db.yugioh-card.com/yugiohdb/member_deck.action?cgid=109ccb9fd6eb6547b3fa1272f5c37c39&dno=25&ope=1&request_locale=en) illustrates the spell-counter and Bamboo engines. Cursed Bamboo Sword and other later support are absent from the local pool; Broken/Golden Bamboo Sword and Magical Citadel are available.
- **Heart of the Underdog.** [Konami's card text](https://www.db.yugioh-card.com/yugiohdb/card_search.action?cid=5837&ope=2&request_locale=en) requires Normal Monster draws during the Draw Phase. This suggests a different, monster-heavy construction that conflicts with the measured lock/draw shell. Reviewed, not duel-tested or ruled out as a theoretical possibility.
- **Recruiters and older loops.** [Konami's Dallas 2011 feature match](https://yugiohblog.konami.com/2011/ycs/11-02-dallas/round-4-feature-match-christopher-bianco-vs-sue-kozarevich/) documents a Cyber Valley/Sangan/Emissary Exodia build. The final deck uses Cyber Valley and Shining Angel; a Sangan/Gold Sarcophagus/Card Trader variant was tested. Card of Safe Return and Butterfly Dagger - Elma are explicitly forbidden in the local list, so their infinite-draw loops were excluded. Modern Treasure Panda/Isolde/Chicken Game engines are outside this card pool.

All legality decisions use `data/wc2011.lflist.conf`, not a present-day tournament ban list. No online win-rate claim substitutes for a local duel benchmark.
