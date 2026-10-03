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
([tier list](../DECK-TIER-LIST.md)).  These are the numbers after the first
duels against a player ([below](#against-a-player)); the decks' first
tournament, in brackets, is kept next to this file.

| # | Deck | Elo | Win rate against the pool [95% CI] | Against the 29 older decks |
|---:|---|---:|---:|---:|
| 1 | Claudi-oh's Countdown | 2024 | 94.0% [93.5, 94.5] (93.6%) | 93.9% (93.5%) |
| 2 | Claudi-oh's Verdict | 1849 | 85.1% [84.3, 85.9] (85.0%) | 87.9% (87.9%) |
| 3 | Machina Gadgets | 1714 | 73.9% [73.0, 74.9] | |
| 4 | Emperor, Arise! | 1687 | 71.2% [70.2, 72.3] | |
| 5 | Gravekeeper's Tomb | 1681 | 70.7% [69.6, 71.7] | |

Before the two decks joined, Machina Gadgets led the pool with 77.6%.

Between the two, the Countdown wins 97.3% of the games: a deck that wins by
attacking has little to say to one that cannot be attacked.

| | Hardest opponents (win rate) |
|---|---|
| Countdown | Burn Princess 46.1%, Gravekeeper's 89.1%, Six Samurai 89.1%, Gladiator Beasts 89.8%, Fortune Ladies 91.8% |
| Verdict | the Countdown 2.7%, Machina Gadgets 66.0%, Fusion Heroes 70.7%, Monarchs 70.7%, Blackwings 73.8% |

The Countdown's 7,680 games: 7,099 won by Final Countdown, 122 because the
opponent ran out of cards, 388 lost on Life Points, 71 lost by running out
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

## Against a player

The first duels a player played against the two decks were read back from
the game's log with `policy-bench replay`
([how](../crates/ygo-policies-bench/README.md#replaying-a-recorded-duel)).
The player won one of two against the Countdown, with a deck built against
it (small Blackwings that attack under the locks, Icarus Attack, Delta Crow -
Anti Reverse, Mystical Space Typhoon, Dust Tornado, Giant Trunade), and two
of two against the Verdict, with the pool's Fusion Heroes.

What the replays showed, and what was done about it:

- **The Countdown Set every Trap it drew.**  In the duel it lost, its hand
  was empty from the third turn on: one Delta Crow in its End Phase took two
  Set Traps, each Icarus Attack two more cards, and the Rainbow Life Set at
  the end had no card to discard.  Covers are now Set one at a time and the
  others kept in hand (Rainbow Life excepted against a burn deck).  Against
  a pilot of the player's list, 69.4% became 74.1% over 4,096 games each;
  against the pool nothing changes (+0.0 points [-0.2, +0.3] over 15,360
  paired games).  (That pilot is a test opponent and not part of this
  repository: its deck is good against the Countdown and little else.)
- **The Verdict never used Solemn Warning on Polymerization.**  The shared
  rule only negated Summons.  It now also negates a Spell or Trap that
  Special Summons, the ones that bring tokens or a small monster excepted:
  over eight decks that play the card, +0.26 points in 61,440 paired games,
  four decks significantly better and none worse.
- **It paid for an answer it had for nothing.**  With Torrential Tribute Set
  next to it, the Verdict paid 2000 Life Points for Solemn Warning on a Normal
  Summon; one window later Torrential Tribute would have destroyed that
  monster and the Elemental HERO Absolute Zero beside it.  A Solemn card now
  leaves a Summon to a Set Bottomless Trap Hole or Torrential Tribute that
  answers it once it is made, when the monster brings no effect of its own
  (it has none, or Skill Drain is face-up).
- **It spent Torrential Tribute one Summon early.**  At 1700 Life Points it
  destroyed a Flip Summoned Armored Bee (1600 ATK), and the Normal Summon
  that followed ended the duel.  On their turn, with their Normal Summon
  still to come and cards in their hand, Torrential Tribute now waits when
  what stands cannot end the duel and has no effect.

The last two rules are there for duels against a player: the benchmark does
not move with them.  For the Verdict against the 29 older decks, over 29,696
paired games each, they are worth -0.03 points [-0.07, +0.02] and -0.02
[-0.05, +0.02], and change how 1.9% and 0.6% of its games are played.  The
policies Summon in another order than the player did: over 3,620 traced
turns of the Verdict's opponents, the Normal Summon or Set came before any
other Summon in 96.2%, and a Summon was followed by the Normal Summon, as in
that duel, in 1.1%.  A unit test holds both positions, and `policy-bench
replay --recheck true` on the recorded duel shows the policy answering them
otherwise now.

What was measured and did not help.  For the Countdown, over 1,024 games
against the pilot of the player's list and 256 against each of the 29 decks
(68.4% and 93.5% before any of it, 73.5% and 93.5% with one cover Set at a
time): two covers Set at a time (70.1% and 93.6%); on top of one at a time,
locks kept in hand until they stop a monster on the field (73.0% and 93.3%),
and Nightmare's Steelcage and Swords of Revealing Light held back while
another cover is ready (71.1% and 93.2%); Skill Drain only in answer to a
monster effect (66.2% and 92.3%).  For the Verdict: keeping in hand, or
Setting, a monster their best one would destroy (87.6% and 87.9% against the
29 decks, from 88.0%).  And the two Trap rules for any monster, with or
without an effect, over every deck that plays those cards: -0.01 points in
57,600 paired games and +0.07 in 42,240.

`policy-bench compare`, each of the 31 policies against the others as they
were (commit 9da85f3) on the same seeds, 128 games per pairing: +0.07 points
over 119,040 paired games, with Machina Gadgets (+0.5), X-Sabers (+0.4) and
Six Samurai (+0.3) significantly better and no deck worse
([summary](duel-replays-vs-old.summary.json),
[metadata](duel-replays-vs-old.metadata.json)).  With 512 games per pairing
for the two decks ([summary](duel-replays-boss-decks-vs-old.summary.json),
[metadata](duel-replays-boss-decks-vs-old.metadata.json)): the Verdict +0.5
points [+0.3, +0.7], the Countdown +0.0 [-0.2, +0.3].

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

The tournament's [raw games](round-robin-2026-10-03-duel-replays.jsonl.gz),
[metadata](round-robin-2026-10-03-duel-replays.metadata.json) and
[summary](round-robin-2026-10-03-duel-replays.summary.json) are next to this
file, and so are those of the decks' first tournament
([raw games](round-robin-2026-10-03-claudi-oh.jsonl.gz),
[metadata](round-robin-2026-10-03-claudi-oh.metadata.json),
[summary](round-robin-2026-10-03-claudi-oh.summary.json)).
