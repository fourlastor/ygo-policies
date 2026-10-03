# Card knowledge for the whole pool: what it changed

The shared card knowledge used to be 34 hand-written entries, taken from the
card scripts of our own decks.  It is now generated: `policy-bench knowledge`
stages each of the 2,444 monsters of the World Championship 2011 pool against
the engine and writes down what happens when it is attacked, aimed at and
attacking ([how](../crates/ygo-policies-bench/README.md#card-facts-from-the-engine),
[the facts in words](../data/wc2011-monster-facts.md)).  The battle planner,
the removal staples and the default target selection use the new facts.

## Calling a battle right

For every attack the probes stage, the shared tactics
(`tactics::default_outcome`) are asked beforehand how it will end, on what the
attacking seat can see; the engine then says how it did end.

| Card facts | Attacks staged | Called right | Attacks printed stats do not explain | Called right |
|---|---:|---:|---:|---:|
| none (empty table) | 74,959 | 96.7% | 2,697 | 10.6% |
| the generated table | 74,959 | 99.7% | 2,697 | 92.9% |

Most of the attacks still called wrong belong to eleven monsters the table
has no room for: their effect depends on the card the opponent draws
(Neo-Spacian Glow Moss), on how many monsters the attacker's side controls
(Cyber Blader), on two things about the attacker at once (Sun Dragon Inti),
or follows the attacker's ATK (Mirage Knight).  The rest are optional effects
the probe always uses and a player would not (D.D. Warrior Lady banishing a
monster it already beat).  The listing's "Left for review" section names the
ones the generator noticed.

The comparison also found a rule the tactics had wrong whatever the card:
two monsters with 0 ATK battling destroy nothing, and were expected to
destroy each other.

## New policies against the old ones

`policy-bench compare`: each deck's new policy and its old one (commit
841490a) play the same seeds against the old policies of the 28 other decks,
256 games per pairing, seats alternating.  The change is the paired difference,
with its 95% interval.

| Deck | Old vs old | New vs old | Change [95% CI] | Games with a different result |
|---|---:|---:|---:|---:|
| crystal | 35.4% | 40.7% | +5.3 [+4.5, +6.0] | 738 of 7,168 |
| monarch | 74.4% | 74.8% | +0.4 [+0.2, +0.6] | 48 of 7,168 |
| heroes | 70.8% | 71.0% | +0.3 [+0.1, +0.4] | 35 of 7,168 |
| gravekeeper | 74.8% | 75.0% | +0.2 [+0.0, +0.3] | 24 of 7,168 |
| x-saber | 57.2% | 57.3% | +0.1 [+0.0, +0.3] | 21 of 7,168 |
| quickdraw-plant | 57.0% | 57.1% | +0.1 [-0.0, +0.3] | 23 of 7,168 |
| pyramid | 31.2% | 31.3% | +0.1 [-0.0, +0.3] | 34 of 7,168 |
| gishki | 49.7% | 49.8% | +0.1 [-0.0, +0.2] | 20 of 7,168 |
| dragunity | 64.0% | 64.1% | +0.1 [-0.0, +0.3] | 30 of 7,168 |
| infernity | 55.2% | 55.3% | +0.1 [-0.0, +0.2] | 29 of 7,168 |
| machina | 78.8% | 78.9% | +0.1 [-0.1, +0.2] | 36 of 7,168 |
| karakuri | 47.3% | 47.3% | +0.1 [-0.1, +0.2] | 29 of 7,168 |
| arcana | 24.0% | 24.0% | +0.1 [-0.2, +0.3] | 67 of 7,168 |
| watt | 20.7% | 20.8% | +0.1 [-0.0, +0.1] | 6 of 7,168 |
| lightsworn | 62.8% | 62.8% | +0.0 [-0.1, +0.2] | 29 of 7,168 |
| harpie | 52.6% | 52.7% | +0.0 [-0.0, +0.1] | 9 of 7,168 |
| tele-dad | 63.8% | 63.8% | +0.0 [-0.2, +0.2] | 32 of 7,168 |
| ojama | 18.6% | 18.6% | +0.0 [-0.1, +0.1] | 8 of 7,168 |
| gladiator | 59.9% | 59.9% | +0.0 [-0.1, +0.1] | 14 of 7,168 |
| blackwing | 69.6% | 69.6% | +0.0 [-0.1, +0.1] | 20 of 7,168 |
| toon | 24.1% | 24.1% | -0.0 [-0.1, +0.0] | 5 of 7,168 |
| fortune-lady | 40.6% | 40.6% | -0.0 [-0.1, +0.1] | 15 of 7,168 |
| draconic-might | 66.7% | 66.7% | -0.0 [-0.1, +0.1] | 19 of 7,168 |
| burn | 28.2% | 28.2% | -0.0 [-0.1, +0.0] | 7 of 7,168 |
| spellcaster | 30.4% | 30.3% | -0.1 [-0.1, +0.0] | 8 of 7,168 |
| morphtronic | 43.0% | 42.9% | -0.1 [-0.2, +0.1] | 28 of 7,168 |
| destiny-hero | 19.6% | 19.5% | -0.1 [-0.2, +0.0] | 12 of 7,168 |
| six-samurai | 67.5% | 67.4% | -0.1 [-0.2, +0.0] | 16 of 7,168 |
| rock-block | 66.4% | 66.3% | -0.1 [-0.3, +0.0] | 48 of 7,168 |

Overall change +0.23 points over 207,872 paired games; significantly better: 5, significantly worse: 0, of 29 decks.

Inside this field the new facts rarely matter: the 29 decks hold few of the
monsters the table is about.  Crystal Beasts gain most because Topaz Tiger's
400 ATK when it attacks now counts: at 2000 it attacks the face-down monsters
that used to stall the deck.

## Against decks built from other cards of the pool

What the table is for is cards our decks do not play.  Two test decks, legal
under the same list, play the 29 policies, old and new, 512 games per
pairing.  Their pilot is a strategy with no deck knowledge (every `Strategy`
hook at its default), from the old library on both sides of the comparison;
the two were registered in a scratch build only, and are not in the repo.

- `exam-walls`, monsters battle does not destroy, always or once a turn:
  Marshmallon, 2 Spirit Reaper, 3 Gellenduo, 3 Dark Resonator, 3 Obnoxious
  Celtic Guard, 2 Gyroid, 2 Shield Wing, 2 Arcana Force 0 - The Fool, with
  3 Gene-Warped Warwolf and 2 Zombyra the Dark to attack;
- `exam-tricks`, monsters that punish an attack: 3 D.D. Warrior Lady, 3 D.D.
  Assailant, 3 Exploder Dragon, 3 Newdoria, 2 Yomi Ship, 2 Wall of Illusion,
  2 Hyper Hammerhead, 2 Amazoness Swords Woman, 2 Reflect Bounder.

Both run the same removal: Mirror Force, Torrential Tribute, 2 Bottomless
Trap Hole, 3 Dimensional Prison, 2 Mystical Space Typhoon, Book of Moon, Dark
Hole, Monster Reborn, 3 Smashing Ground, and Fissure (3 with the tricks, 1
and Swords of Revealing Light with the walls).

| Test deck | Old policies | New policies | Change [95% CI] | Policies better / worse | Turns old / new |
|---|---:|---:|---:|---:|---:|
| exam-walls | 33.9% | 35.6% | +1.7 [+1.3, +2.0] | 13 / 0 of 29 | 23.5 / 23.1 |
| exam-tricks | 34.5% | 35.2% | +0.7 [+0.2, +1.1] | 5 / 2 of 29 | 26.0 / 26.2 |

Against the walls, Six Samurai and Harpie gain most (+4.7 each), then Machina
and Dragunity (+3.3).  Against the tricks, Crystal Beasts gain 6.6, Heroes
4.3, Blackwing 4.1 and Monarch 3.3; Gravekeeper's and Spellcaster lose 2.9.

Against the walls the facts pay: policies stop attacking what battle does not
destroy and spend two attacks on what survives one.  Against the monsters
that take their attacker with them there is little to gain: the right play
is mostly the one the old policies made without knowing, attack and trade
one for one.  A first version that declined such trades for a more valuable
attacker lost 2.6 points to this deck (Monarch 12.7), sitting behind a
stalemate the other side's removal then broke; the trade is now taken unless
it costs more than the attacker.

Both test decks are hard for the field as it is: with a pilot that knows
nothing about them, they win about two games in three.

## In real games

Ally of Justice Catastor destroys the non-DARK monster that attacks it before
damage calculation.  In traced games of Monarch, Machina, Heroes,
Gravekeeper's and Blackwing against five decks that Synchro Summon it, the
old policies declared 7 attacks at a Catastor with a non-DARK monster in 400
games, the new ones none.

## What the probes do not show

A staged board shows one situation.  Not in the table, and so played as
before: effects set up by a proper Summon (a coin toss, counters), effects
that need particular cards in hand, Deck or Graveyard, effects on other
monsters of the same side, and Spells and Traps altogether.  A monster that
leaves the field to negate (Stardust Dragon, Tytannial) gets no "negates"
fact: making it leave is a trade the old policies already took, and taking it
away cost up to 15 points against Quickdraw Plants in a first version.

