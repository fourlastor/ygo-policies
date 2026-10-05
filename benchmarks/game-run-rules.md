# Rules from a recorded run of the game

*Sands of the Duel* records every duel.  One run of six duels, played against
the policies of commit 841490a, was read for the mistakes of the opponents:
Infernity, Fortune Lady and Draconic Might made clear ones, Crystal Beasts
one, Destiny HERO and Gishki lost to their cards more than to their play.
This is what came of checking each mistake against the policies of today,
and of a search on top of the three pilots afterwards.

Every number here is from the standard format: 8000 Life Points, seats
alternating.  The game's own format (the player first, the opponent at 4000)
decided nothing.  The numbers were taken while the rules were written:
`policy-bench compare` of a library with one rule against the same library
without it, on the same seeds, against the 12 reference decks.

## What the run showed

| Deck | In the recorded duel | Cause | Now |
|---|---|---|---|
| Infernity | Launcher on the field, Avenger and Plaguespreader Zombie in hand: Avenger was Normal Summoned and the turn ended | Launcher's discard was only used with two Infernity monsters in hand | Launcher sends the Infernity monster away, the other takes the Normal Summon ([below](#infernity)) |
| Infernity | Avenger (0 ATK) stood face-up in Attack Position; no monster was ever Set | every Normal Summon scored 500, every Set less | the small monsters go face-down unless the hand empties and something follows |
| Fortune Lady | Book of Moon stayed in hand for the whole duel | the deck's rule Set Traps only | [Quick-Play Spells are Set](#quick-play-spells-are-set), in every deck that held one |
| Draconic Might | Smashing Ground took Red-Eyes Darkness Metal Dragon with Solemn Judgment Set | the Life Point floor | fixed before this ([solemn-price.md](solemn-price.md)) |
| Draconic Might | the Dragon attacked directly first, Tragoedia came down, Vanguard of the Dragon could not attack any more | direct attacks went strongest first | [the weakest attacks first](#the-weakest-attacks-first-the-answer-waits-for-the-strongest), for every deck |
| Crystal Beasts | Emerald Tortoise (600 ATK, 2000 DEF) was Normal Summoned on an empty field and took 1500 | a tie: the deck scores Setting it 1300, the shared rule scores Summoning it on an empty field 1300, and the engine lists the Summon first | unchanged: always Setting it gains nothing ([below](#what-did-not-hold)) |
| Destiny HERO | Over Destiny brought a Doom Lord that the End Phase destroyed | it was played for any monster on an empty field | only as a Tribute for Dogma or Plasma |
| Destiny HERO | D - Time never left the hand | it needs an Elemental HERO, and the list has none | unchanged: a card of the list, not a play |
| Gishki | nothing | | |

## Quick-Play Spells are Set

A Quick-Play Spell can be played in the opponent's turn only when it is Set.
The shared rule Sets Traps and Quick-Play Spells, but twenty-one decks had
their own rule, which said "Traps" (some added a card or two of their own).  Their
Book of Moon, Shrink and Enemy Controller waited in hand for a use in their
own turn, and the shared rules that answer an attack with them never ran.

Those rules are gone; a deck now says how to Set a card only where it has a
reason (Tower of Babel, D - Time, Super Polymerization, Monarch's empty
backrow for Treeborn Frog).

Each card on its own, in points of win rate, for the decks that held it
(256 games per pairing, Enemy Controller 512):

| Deck | Book of Moon | Shrink | Enemy Controller | Typhoon |
|---|---:|---:|---:|---:|
| gladiator |  | +4.5 |  | +0.2 |
| karakuri | +1.7 |  | +2.0 | +0.0 |
| quickdraw-plant | +0.8 |  | +1.7 | +0.4 |
| tele-dad | +0.6 |  | +1.7 | +0.3 |
| machina | +1.3 |  |  | +1.1 |
| fortune-lady | +0.8 | +1.1 |  | +0.2 |
| toon |  | +1.4 |  | +0.2 |
| x-saber | +1.4 |  |  | +0.1 |
| arcana | +0.5 | +1.1 |  | -0.1 |
| gravekeeper | +1.1 |  |  | +0.3 |
| pyramid | +0.7 | +0.8 |  | -0.1 |
| destiny-hero | +1.2 |  |  | +0.2 |
| harpie | +0.9 |  |  | +0.4 |
| six-samurai | +1.2 |  |  | +0.0 |
| dragunity |  | +0.7 |  | +0.3 |
| spellcaster | +0.7 |  |  |  |
| morphtronic |  |  |  | +0.4 |
| watt |  |  |  | +0.2 |
| heroes |  |  |  | +0.1 |
| gishki |  |  |  | -0.1 |
| **all that held it** | **+0.99** [+0.81, +1.18] | **+1.59** [+1.33, +1.85] | **+1.82** [+1.50, +2.13] | **+0.22** [+0.12, +0.31] |

Thirteen decks held Book of Moon, six Shrink, three Enemy Controller,
nineteen Typhoon.  Every deck gains from Book of Moon, Shrink and Enemy
Controller; the three minus signs under Typhoon are well inside their
intervals.

## The weakest attacks first, the answer waits for the strongest

Two rules, one seen from each side of a Battle Phase.

**Attacking.**  On an open field the strongest monster attacked first.  Now
the weakest does, unless one attack ends the duel: what the first hit brings
out or sets off (Tragoedia, Gorz, a Trap), the stronger attackers are still
there to meet.

**Defending.**  Dimensional Prison, Sakuretsu Armor, Book of Moon, Compulsory
Evacuation Device, Enemy Controller, Magic Cylinder and Draining Shield each
stop one attacker, and were played on the first attack that hurt.  Against an
opponent who attacks weakest first that is the wrong monster.  They now let
an attack through while stronger monsters of theirs can still attack this
turn, as many as there are answers; never an attack that ends the duel.

With them, Dust Tornado takes a face-up Spell or Trap their deck runs on at
the first chance, as Typhoon has since [the search rules](search-rules.md).

Each rule alone, every deck against the 12 reference decks, 256 games per
pairing (99,840 paired games):

| | Change [95% CI] | Decks significantly better | Worse |
|---|---:|---:|---:|
| The weakest attacks first | +0.25 [+0.20, +0.29] | 16 of 31 | 0 |
| The answer waits for the strongest | +0.23 [+0.17, +0.28] | 10 of 31 | 0 |

The waiting was measured with the first rule on both sides: against
opponents that attack strongest first there is nothing to wait for.  Most
for Karakuri (+1.1), Fortune Lady (+0.7), Six Samurai and Machina (+0.6);
the attack order most for Harpie (+0.7) and Morphtronic (+0.5).  Dust
Tornado's rule is measured with Infernity below.

## Infernity

Almost every Infernity effect needs an empty hand.  The pilot knew that, and
still left cards in it.

From the recorded duel:

- Launcher sends an Infernity monster to the Graveyard whenever the Normal
  Summon has another monster to take (+0.4);
- Mirage, Avenger, Plaguespreader Zombie and Glow-Up Bulb have 0 to 400 ATK
  and nothing but their effects.  Mirage is Normal Summoned only when this
  empties the hand and two monsters wait in the Graveyard; the Tuners when a
  Synchro Summon follows.  Otherwise they are Set, and a stronger monster
  takes the Normal Summon.  Most of the gain is Mirage: with two in the
  Graveyard it used to take the Normal Summon whatever stayed in hand, and
  stood there with 0 ATK and no effect (+3.2).

From a search on top of that pilot (`policy-bench search`, 384 games: the
pilot alone 49.2%, with the search 75.5%):

- Stygian Street Patrol in the Graveyard Special Summons a Fiend from the
  hand: one card less in hand, and no Normal Summon spent.  The pilot never
  used it (+2.9);
- Archfiend's search takes a card that leaves the hand again this turn: the
  Launcher, a monster while the Normal Summon is unspent, Infernity Force
  otherwise; with nothing of the kind in the Deck the effect is declined.
  It used to take the dearest card, which then sat in the hand and switched
  everything off (+2.8);
- a small Tuner beside a non-Tuner of ours is Normal Summoned for the Synchro
  Summon, whatever stays in hand (about +2.3);
- Dust Tornado at the first chance, by the shared rule above (+0.8);
- Launcher is given for two monsters, not for one (+0.5);
- Guardian goes face-down under a stronger monster: in Attack Position its
  1200 ATK takes the damage of every attack (+0.4).

The gains are each rule's own when it was added (12,288 paired games each).
Together, Infernity against the 12 reference decks:

| | Games | Before | After | Change [95% CI] |
|---|---:|---:|---:|---:|
| The rules from the recorded duel, and the attack order | 3,072 | 49.1% | 53.7% | +4.6 [+3.3, +5.9] |
| Then the rules from the search, and the waiting | 12,288 | 52.6% | 62.5% | +9.9 [+9.2, +10.7] |

(The second line starts below where the first ends: its opponents play the
shared rules too.)  Destiny HERO's Over Destiny is +0.14 [+0.02, +0.25] over
13,312 games.

The search can show an effect the pilot never activates, as with Patrol.  It
cannot show a better choice of card: Archfiend's search came from asking why
the search kept declining it.

## Fortune Lady

A search on the pilot of the commits above (416 games): 34.5% alone, 54.0%
with the search.

- The Normal Summon is never left unused.  With Light or Fire as the only
  monsters in hand the pilot ended its turn on an empty field of its own
  whenever the other field was empty too, and with a monster of its own it
  kept Light, Fire, Wind and Water in hand unless they could be Summoned in
  Attack Position.  Now Light is Normal Summoned face-up, before Wind and
  Water (it grows, and an effect that removes it brings another Lady from
  the Deck), Fire is Set, and Wind and Water are Set under a stronger
  monster (+5.7 [+5.1, +6.4]; Setting Light instead is +3.6);
- a Tribute Summon of Dark or Earth never gives a grown Lady for a smaller
  one, and Dark goes before Earth when it wins a battle on arrival: its
  effect brings the Tribute back (+1.6 [+1.3, +2.0]).

Together 34.1% to 41.5% against the reference decks (+7.4 [+6.7, +8.1],
13,312 paired games).

## Draconic Might

The same search (416 games): 58.2% alone, 71.6% with it, the smallest gap of
the three decks.

- Dark Hole is kept for two monsters, or one of 2400 ATK and more.  With an
  empty field of its own the pilot spent it on any single monster
  (+0.5 [+0.2, +0.7]);
- Armed Dragon LV5 is Tribute Summoned under a bigger monster when a monster
  in hand pays for its effect, which destroys the bigger one at once.  With
  2400 ATK it used to wait in hand (+0.4 [+0.3, +0.5]).

Together 58.9% to 59.8% (+0.9 [+0.6, +1.1], 13,312 paired games).  Two of
the search's most frequent answers are in the table below: as rules they
lose.

One more for every deck with Shrink: a direct attack that would end the duel
is halved (23 games of 38,912 differ, all won).

## What did not hold

Paired, against the 12 reference decks:

| Candidate | Change [95% CI] |
|---|---:|
| Crystal Beasts: Emerald Tortoise is always Set | -0.16 [-0.40, +0.07] |
| Every deck: a monster with low ATK and much more DEF is Set on an empty field too | -0.14 [-0.20, -0.09] |
| Infernity: Guardian is always Set | -0.12 |
| Infernity: leftover Spells are Set before the Battle Phase, so the hand is empty for it | +0.03 [-0.08, +0.15] |
| Infernity: Glow-Up Bulb returns from the Graveyard only for a Synchro Summon | -0.17 [-0.41, +0.06] |
| Infernity: Beetle is Set while another monster stays in hand | +0.24 [-0.25, +0.73] |
| Fortune Lady: Wind is Set unless its effect or a battle is there to win (three forms) | -1.80 to -0.10 |
| Fortune Lady: Light before Summoner Monk too | +0.07 [-0.12, +0.26] |
| Fortune Lady: a Shrink or Book of Moon in hand counts when choosing the Summon | +0.14 [-0.08, +0.35] |
| Every deck with Shrink: on our own turn, to get an attack over a bigger monster | -0.04 [-0.14, +0.07] |
| Every deck: Call of the Haunted before their attacks, on an empty field | -0.04 [-0.13, +0.05] (Fortune Lady) |
| Draconic Might: Call of the Haunted from the start of their turn (three forms) | -0.34 to -0.20 |
| Draconic Might: Red-Eyes Darkness Metal Dragon waits on a first turn | -0.44 [-0.75, -0.12] |
| The five decks with Solemn Judgment: also against Summons of 1700 ATK and more | +0.07 [-0.08, +0.22] |

The last three are what the search did most often with Draconic Might: it
held Red-Eyes Darkness Metal Dragon back on a first turn in seven games (z
up to 3.8), played Call of the Haunted early in thirteen and Solemn Judgment
against an ordinary Summon in eleven.  Right in those positions, wrong or
nothing as habits, as with Blackwing and Monarch before.

The Tortoise is the mistake of the recorded duel that is none here.  A
Crystal Beast that is destroyed goes to the Spell & Trap Zone, where the deck
wants it; the 1500 Life Points buy that and 600 damage.  At 8000 it comes out
even.  The second line is the same idea for every deck, and it costs the
decks whose small monsters do their work face-up (Six Samurai -2.3,
Lightsworn -1.4).

## Records

- Unit tests: `quick_play_spells_are_set`,
  `the_weakest_attacks_first_and_the_answer_waits_for_the_strongest`,
  `infernity_plays_toward_an_empty_hand`,
  `fortune_lady_uses_its_normal_summon`,
  `draconic_might_plays_what_the_search_found`.
- The logged searches are 10 to 16 MB each and are not kept here.
  `policy-bench search --policies infernity --opponents existing --games 32
  --log true`, with the `--core` build the search needs, writes one again on
  the commit before that deck's rules.
- One comparison of every deck against the 30 others, and the tier list, are
  made once for all of this work and are not in this commit.
