# Solemn Judgment and Solemn Warning: a price, not a floor

Solemn Judgment costs half the Life Points its player has, whatever they are:
it can always be paid, and the fewer there are the less it costs.  The
policies only played it at 4000 Life Points and more (the shared rule in
`staples.rs`, and a copy each in Draconic Might and Lightsworn).  An ordinary
opponent in *Sands of the Duel* starts at exactly 4000, so there the card was
dead after the first damage.  In a recorded duel Draconic Might Set it on
turn 2 at 4000, lost Red-Eyes Darkness Metal Dragon to Smashing Ground on
turn 3, and lost on turn 5 with it still Set.

## The rule now

One shared rule (`staples::judgment`); the two deck copies are gone.  The
price is half the current Life Points, and it is weighed against what the
answer saves:

- **A Summon** is negated when the monster is big (2400 ATK and more, or from
  the Extra Deck) at any Life Points, as before, or when its ATK is at least
  the price: its one attack would cost as much as the Trap does.  Not when a
  Set Bottomless Trap Hole or Torrential Tribute answers it for nothing.
- **A Spell or Trap** is negated when what it would take from us
  (`staples::taken_by`) is worth at least the price, and more than the Trap
  given for it.  A wipe counts our cards it destroys less their own; a
  removal counts what it targets, or what its text picks (Smashing Ground the
  highest DEF, Fissure the lowest ATK).  Heavy Storm on the Trap alone takes
  nothing that was not lost anyway.
- **Next to Solemn Warning**, the cheaper of the two goes first.

Solemn Warning had a floor too: it was played above 3000 Life Points only,
so from 4000 it died after 1000 damage.  It is now played whenever paying its
2000 leaves any (also in Rock Block's own copy of the rule).

## Measured

Both tables compare the policies of this commit with the ones before it on
the same seeds.  Eleven decks play one of the two cards in the main deck; the
twenty others play every game as before (0 of 76,800 differ).

### As the game plays it: the deck second, at 4000 Life Points

`policy-bench matchup --first PLAYER --lp 8000,4000`, 2,048 games per pairing,
with Blackwing, Heroes and Monarch as the first player (none of them plays
either card in the main deck, so their own play is the same on both sides).

| Deck | Before | Now | Change [95% CI] | Games with a different result |
|---|---:|---:|---:|---:|
| verdict | 48.4% | 50.9% | +2.5 [+1.9, +3.0] | 312 of 6,144 |
| machina | 42.7% | 45.1% | +2.3 [+1.8, +2.8] | 237 of 6,144 |
| tele-dad | 26.0% | 27.6% | +1.7 [+1.2, +2.1] | 172 of 6,144 |
| lightsworn | 28.8% | 29.8% | +0.9 [+0.6, +1.3] | 130 of 6,144 |
| draconic-might | 33.4% | 34.1% | +0.7 [+0.4, +1.0] | 78 of 6,144 |
| gravekeeper | 37.3% | 37.8% | +0.5 [+0.2, +0.7] | 42 of 6,144 |
| rock-block | 19.0% | 19.2% | +0.3 [-0.0, +0.6] | 91 of 6,144 |
| six-samurai | 33.2% | 33.4% | +0.3 [+0.1, +0.4] | 19 of 6,144 |
| quickdraw-plant | 26.6% | 26.8% | +0.2 [+0.0, +0.4] | 30 of 6,144 |
| x-saber | 22.1% | 22.2% | +0.1 [-0.0, +0.2] | 16 of 6,144 |
| gladiator | 14.7% | 14.8% | +0.1 [-0.1, +0.2] | 17 of 6,144 |

Together +0.86 points [+0.77, +0.96] over 67,584 paired games: +1.03 against
Blackwing, +0.54 against Heroes, +1.02 against Monarch.

### At 8000 Life Points, seats alternating

`policy-bench compare`, each deck against the 30 others, 128 games per
pairing.

| Deck | Before | Now | Change [95% CI] | Games with a different result |
|---|---:|---:|---:|---:|
| machina | 72.9% | 73.9% | +1.0 [+0.6, +1.4] | 68 of 3,840 |
| verdict | 85.1% | 85.6% | +0.5 [+0.2, +0.9] | 37 of 3,840 |
| lightsworn | 60.1% | 60.4% | +0.3 [+0.1, +0.6] | 29 of 3,840 |
| draconic-might | 63.5% | 63.7% | +0.3 [+0.1, +0.5] | 14 of 3,840 |
| tele-dad | 60.5% | 60.7% | +0.2 [-0.2, +0.6] | 58 of 3,840 |
| rock-block | 62.9% | 63.2% | +0.2 [+0.0, +0.4] | 12 of 3,840 |
| gravekeeper | 71.0% | 71.0% | +0.1 [-0.0, +0.2] | 3 of 3,840 |
| gladiator | 55.7% | 55.7% | +0.1 [-0.0, +0.2] | 3 of 3,840 |
| six-samurai | 63.2% | 63.3% | +0.1 [-0.0, +0.1] | 2 of 3,840 |
| quickdraw-plant | 52.8% | 52.9% | +0.1 [-0.0, +0.1] | 2 of 3,840 |
| x-saber | 52.7% | 52.7% | +0.0 [-0.1, +0.1] | 4 of 3,840 |

Over these eleven decks +0.26 points [+0.19, +0.33]; five significantly
better, none worse.

## The parts, one at a time

Each part was measured on its own while the rule was written, the same way,
on the working tree of that day: the five decks that play Solemn Judgment
against the 12 reference decks at 8000 Life Points, and second at 4000
against three to five first players, the game's starter deck among them.
The sizes are what matters here, not the last digit.

| Part | Second at 4000 | At 8000 |
|---|---:|---:|
| The floor removed, nothing else | +0.88 [+0.81, +0.95] | +0.27 [+0.22, +0.31] |
| Also a Summon whose ATK is at least the price | +0.44 [+0.35, +0.53] | +0.04 [-0.01, +0.09] |
| Also removal of one card worth at least the price | +0.28 [+0.20, +0.35] | +0.05 [-0.00, +0.10] |
| Wipes weighed against the price; Lightsworn on the shared rule | +0.11 [+0.06, +0.16] | +0.07 [+0.00, +0.14] |
| Solemn Warning first while it is the cheaper | (price never above 2000) | +0.09 [+0.04, +0.13] |
| Solemn Warning whenever it can be paid (nine decks) | +0.22 [+0.16, +0.27] | no change |

What was tried and lost:

- every Summon, the big ones too, must match the price: -0.54 [-0.67, -0.42]
  at 8000.  The big Summons are worth half of 8000 Life Points;
- a more willing bar, ATK at least 0.6 of the price: -0.28 [-0.39, -0.16]
  second at 4000;
- a minimum of 1900 ATK for the Summons below 2400: +0.18 instead of +0.44.
  A minimum of 1500, 1000 or none makes no difference, so there is none.

## What it does not do

- `taken_by` knows the wipes and about twenty removal cards of the era, by
  name.  A removal that is not in it is still not answered.
- A free answer of our own, like Stardust Dragon on the field, does not yet
  go before the Trap.

## Records

- `solemn-price-vs-old.*`: the comparison at 8000 (seeds, libraries, card
  database and scripts in the metadata, per-pairing results in the summary).
- `solemn-price-second-at-4000-{old,new}-vs-{blackwing,heroes,monarch}.*`:
  the six runs of the first table, with each pairing's result.  The paired
  intervals come from their game rows, which the command in the metadata
  writes again.
- A unit test, `solemn_cards_weigh_their_price`, holds the recorded duel's
  position and the cases above.
