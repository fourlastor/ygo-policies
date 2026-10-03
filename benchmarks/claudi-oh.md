# Claudi-oh's decks: how they were built and what they score

Two decks were built for the boss of *Beat Claudi-oh*, each with its own
policy, to rank above every other deck in the pool.  Both are legal under
the World Championship 2011 Forbidden & Limited list, and both play by the
rules every policy plays by: they see what their seat sees.

- **Claudi-oh's Countdown** (`countdown`) does not attack.  Final Countdown
  wins the duel 20 turns after it resolves, and the rest of the deck keeps
  the opponent's ten Battle Phases from mattering.
- **Claudi-oh's Verdict** (`verdict`) attacks.  Skill Drain switches every
  monster effect off, and its monsters are the bigger ones once it has.

## Result

`policy-bench round-robin`, 31 policies, 256 games per pairing, seats
alternating: 119,040 games, no failures, one draw
([tier list](../DECK-TIER-LIST.md)).

| # | Deck | Elo | Win rate against the pool [95% CI] | Against the 29 older decks |
|---:|---|---:|---:|---:|
| 1 | Claudi-oh's Countdown | 2011 | 93.6% [93.1, 94.1] | 93.5% |
| 2 | Claudi-oh's Verdict | 1848 | 85.0% [84.2, 85.8] | 87.9% |
| 3 | Machina Gadgets | 1710 | 73.6% [72.6, 74.5] | |
| 4 | Emperor, Arise! | 1687 | 71.2% [70.2, 72.3] | |
| 5 | Gravekeeper's Tomb | 1682 | 70.8% [69.8, 71.8] | |

Before the two decks joined, Machina Gadgets led the pool with 77.6%.

Between the two, the Countdown wins 97.3% of the games: a deck that wins by
attacking has little to say to one that cannot be attacked.

| | Hardest opponents (win rate) |
|---|---|
| Countdown | Burn Princess 45.7%, Gladiator Beasts 88.3%, Gravekeeper's 89.1%, Six Samurai 89.1%, Tele-DAD 89.8% |
| Verdict | the Countdown 2.7%, Machina Gadgets 67.2%, Fusion Heroes 69.5%, Monarchs 70.3%, Blackwings 74.2% |

The Countdown's 7,680 games: 7,057 won by Final Countdown, 133 because the
opponent ran out of cards, 426 lost on Life Points, 64 lost by running out
of cards before the clock did.  They last 26 turns on average; the
Verdict's last 12.

## The Countdown

The list: 3 Final Countdown, 3 Pot of Duality, 3 Upstart Goblin; 3 Messenger
of Peace, 1 Level Limit - Area B, 1 Gravity Bind, 3 Skill Drain; 3 Nightmare's
Steelcage, 1 Swords of Revealing Light; 3 Threatening Roar, 1 Waboku, 3
Rainbow Life; 3 Battle Fader, 3 Swift Scarecrow, 3 Cyber Valley, 3 Shining
Angel.

Each step below was measured against the 29 older decks (128 or 256 games
per opponent) before it was kept, and read off traced games before it was
tried.

| Step | Win rate |
|---|---:|
| First list: the locks, one-turn Traps, Lava Golem, Gold Sarcophagus | 76.9% |
| Three mistakes of the pilot (Gravity Bind was Set and never flipped; a cover that got negated still counted as covering the turn; Gold Sarcophagus sat idle once the clock ran) | 83.6% |
| Skill Drain | 87.4% |
| Cyber Valley | 91.1% |
| Shining Angel for Lava Golem, Rainbow Life for Thunder of Ruler | 92.9% |
| A negation seen in the chain is answered with the next cover; monsters leave the hand under Ominous Fortunetelling | 93.3% |
| The third Skill Drain | 94.2% |
| Final (the other policies now know what Cyber Valley is worth to this deck) | 93.5% |

What the traced games showed, and what was done about it:

- **The wall fell to monster effects.**  In the games the first list lost,
  the opponent activated, per game: Scrap Dragon 2.4 to 4.0 times (Tele-DAD,
  Karakuri, X-Sabers), Gravekeeper's Descendant 3.2, Evigishki Soul Ogre 4.2,
  Breaker the Magical Warrior 3.9, Shi En 2.8, Heraklinos 2.7.  All of them
  are effects of monsters on the field, which is what Skill Drain switches
  off: against the same twelve opponents a Messenger of Peace then stays 4.8
  of their turns instead of 3.4, and Skill Drain itself 7.  Chained to an
  effect aimed at Skill Drain itself, it negates that effect and stays.
- **Covers ran out, not time.**  The win rate hardly depends on when Final
  Countdown is activated (94.0% on the first turn, 95.3% on the seventh or
  later, over 960 traced games): the deck draws about one turn of cover per
  card, and loses when a removal-heavy opponent takes more than that.  Cyber
  Valley is a cover that draws its own replacement, which is why three
  copies were worth 3.7 points.
- **The Lava Golem we gave them did the damage.**  In 67 lost games, the
  card most often on the opponent's field in a turn the wall fell was our own
  Lava Golem (43 such turns; Shi En, next, 5).  A 3000 ATK monster for two of
  theirs is fine behind a lock and fatal once the lock is gone.  Shining
  Angel in its place brings a Cyber Valley from the Deck when it is
  destroyed.
- **Burn was the lost matchup** (23%): effect damage, on top of 2000 Life
  Points for Final Countdown and 1000 for Skill Drain.  Rainbow Life chained
  to a burn Spell or Trap turns it into Life Points, and Skill Drain is no
  longer paid for against a burn deck unless Fire Princess is doing the
  burning: 46%.  It is still the one opponent the deck loses to.

What was measured and did not help: taking small hits early to save covers
for later (no gain, and worse against burn), The Dark Door, Scrap-Iron
Scarecrow, Draining Shield, Dimension Wall and Magic Cylinder in place of
whole-turn covers (worse: one attack stopped is not a turn stopped), Zero
Gardna, Nimble Momonga, Machine Duplication (worse), Mystical Space Typhoon
(worse), Des Wombat (burn up, everything else down), fewer than three Final
Countdowns (worse), Skill Drain kept face-down until a monster effect is
activated (no better).

## The Verdict

The list: 3 Beast King Barbaros, 3 Thunder King Rai-Oh, 3 Doomcaliber
Knight, 3 Gene-Warped Warwolf, 2 Chainsaw Insect, 2 Goblin Attack Force; 3
Pot of Duality, Dark Hole, Monster Reborn, 2 Mystical Space Typhoon, Book of
Moon, 2 Smashing Ground; 3 Skill Drain, Solemn Judgment, 2 Solemn Warning, 2
Bottomless Trap Hole, 3 Dimensional Prison, Mirror Force, Torrential Tribute,
Trap Dustshoot.

Its first list scored 86.8% against the 29 older decks with a pilot of 150
lines: the shared agent already plays the Traps and the battles.  Variants
moved it between 84% and 88%.  What was kept: Skill Drain face-up at the
first chance rather than when the board calls for it, Doomcaliber Knight
instead of Cyber Dragon and Gorz, and Barbaros Summoned without Tributes
even next to our own monsters (the engine asks which way: the pilot answers).
Skill Drain is the deck: against its four hardest opponents it was activated
0.86 times a game in the traced games the deck won and 0.58 in those it lost.

## Shared code

Three changes reach every policy:

- `Ctx::facts` gives a face-up monster no on-field effects while a Skill
  Drain is face-up (what it does from the Graveyard stays).  Before, a
  policy under its own Skill Drain still played around Jinzo or Horus LV8.
- Messenger of Peace, Level Limit - Area B and Nightmare's Steelcage are
  attack locks to everyone (`staples::ATTACK_LOCKS`): the first thing to
  destroy, as Swords of Revealing Light and Gravity Bind already were.
  Cyber Valley is in `knowledge::owner_worth`, so the other policies value
  removing it.
- `Memory::activated`: the effects a policy activated this turn.

`policy-bench compare`, each of the 29 older policies against the others on
the same seeds, new library against the old one (commit 2a53066): no change
(0.00 points over 103,936 paired games; 72 games ended differently; no deck
significantly better or worse;
[summary](claudi-oh-shared-code-vs-old.summary.json),
[metadata](claudi-oh-shared-code-vs-old.metadata.json)).

## What the numbers do not say

The opponents are the other policies.  Over 192 traced games they attacked
a face-up Cyber Valley in 130 of the 136 Battle Phases they could, as a
player would, but a field of face-down monsters in only 102 of 405: a Set
Shining Angel is more of a wall against them than against a player, who
attacks it and then meets the Cyber Valley it brings.  And they do not
side-deck: the cards a player would bring in against the Countdown (Mystical
Space Typhoon, Dust Tornado, Royal Decree, Trap Stun) are in their lists in
ones and twos, if at all.

## Reproducing

```bash
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
# One deck against chosen opponents.
target/release/policy-bench matchup --policies countdown,verdict --opponents all \
  --games 256 --output claudi-oh.jsonl
# The tournament behind the tier list.
target/release/policy-bench round-robin --policies all --games 256 --seed 730000 \
  --output tournament.jsonl --markdown DECK-TIER-LIST.md
```

The tournament's [raw games](round-robin-2026-10-03-claudi-oh.jsonl.gz),
[metadata](round-robin-2026-10-03-claudi-oh.metadata.json) and
[summary](round-robin-2026-10-03-claudi-oh.summary.json) are next to this file.
