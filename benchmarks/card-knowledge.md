# Card knowledge: new policies against the old ones

Shared card knowledge (`crates/ygo-policies/src/knowledge.rs`) gives every
policy facts that printed stats do not show, looked up only by the code the
seat can see (the opponent's face-down cards stay unknown):

- monsters battle cannot destroy (Arcana Force 0 - The Fool, Spirit Reaper,
  Blackwing Armor Master, X-Saber Pashuul, Marshmallon, Infernity Guardian
  with an empty hand, Morphtronics behind a Defense Position Boarden);
- targeting: The Fool's coin, White Night Dragon against Spells and Traps,
  Spirit Reaper dying to any targeting effect, Toon Kingdom protecting Toons;
- what battle destruction gives back (floaters, searches, Machina Fortress);
- the Damage Step: Jain and Galahad's bonus, Galahad's malus, D.D. Warrior
  Lady's banishing, the Karakuri switching position when attacked, attacks
  Krebons or a Defense Position Boomboxen can negate;
- what a face-up card is worth to the deck that plays it, as that deck's own
  policy rates it (Toon World, Necrovalley, Gateway of the Six, Pyramid of
  Light, Lumina...), so the shared removal goes for it first.

The attack planner, threat and sweep worth, the targeting staples and
MST/Dust Tornado use them; a deck still overrides any of it in its own hooks.

## Measurement

`policy-bench compare`: each deck's new policy and its old one (`main` before
this change) play the same seeds against the old policies of the 28 other
decks, 256 games per pairing, seats alternating. The change is the paired
difference, with its 95% interval.

| Deck | Old vs old | New vs old | Change [95% CI] | Games with a different result |
|---|---:|---:|---:|---:|
| lightsworn | 60.0% | 62.3% | +2.3 [+1.6, +3.0] | 577 of 7,168 |
| monarch | 72.6% | 74.1% | +1.5 [+0.9, +2.1] | 515 of 7,168 |
| draconic-might | 65.3% | 66.6% | +1.3 [+0.9, +1.8] | 269 of 7,168 |
| quickdraw-plant | 56.1% | 57.1% | +0.9 [+0.5, +1.4] | 278 of 7,168 |
| pyramid | 31.2% | 32.1% | +0.9 [+0.5, +1.3] | 243 of 7,168 |
| infernity | 56.0% | 56.8% | +0.9 [+0.5, +1.3] | 218 of 7,168 |
| x-saber | 56.2% | 57.0% | +0.8 [+0.3, +1.2] | 256 of 7,168 |
| heroes | 71.1% | 71.9% | +0.8 [+0.4, +1.1] | 178 of 7,168 |
| crystal | 35.8% | 36.5% | +0.7 [+0.4, +1.0] | 140 of 7,168 |
| gishki | 48.7% | 49.3% | +0.7 [+0.2, +1.1] | 271 of 7,168 |
| fortune-lady | 39.8% | 40.5% | +0.6 [+0.2, +1.1] | 284 of 7,168 |
| spellcaster | 29.6% | 30.1% | +0.5 [+0.2, +0.8] | 131 of 7,168 |
| watt | 20.3% | 20.7% | +0.5 [+0.1, +0.8] | 135 of 7,168 |
| machina | 78.5% | 78.9% | +0.4 [-0.1, +0.8] | 270 of 7,168 |
| harpie | 52.1% | 52.5% | +0.4 [-0.0, +0.8] | 205 of 7,168 |
| burn | 27.7% | 28.1% | +0.4 [+0.2, +0.5] | 37 of 7,168 |
| toon | 24.9% | 25.2% | +0.3 [+0.0, +0.5] | 82 of 7,168 |
| morphtronic | 44.0% | 44.2% | +0.2 [-0.2, +0.6] | 197 of 7,168 |
| gladiator | 59.8% | 60.0% | +0.2 [-0.2, +0.6] | 247 of 7,168 |
| arcana | 23.4% | 23.6% | +0.2 [-0.2, +0.5] | 158 of 7,168 |
| destiny-hero | 18.9% | 19.0% | +0.1 [-0.3, +0.4] | 177 of 7,168 |
| blackwing | 69.1% | 69.1% | +0.0 [-0.5, +0.6] | 382 of 7,168 |
| tele-dad | 63.3% | 63.3% | +0.0 [-0.5, +0.5] | 333 of 7,168 |
| six-samurai | 67.6% | 67.6% | +0.0 [-0.4, +0.4] | 201 of 7,168 |
| ojama | 18.2% | 18.2% | +0.0 [-0.4, +0.4] | 168 of 7,168 |
| dragunity | 64.1% | 64.1% | -0.0 [-0.4, +0.3] | 190 of 7,168 |
| rock-block | 66.4% | 66.3% | -0.2 [-0.6, +0.2] | 217 of 7,168 |
| karakuri | 50.3% | 50.1% | -0.2 [-0.6, +0.2] | 233 of 7,168 |
| gravekeeper | 76.5% | 76.2% | -0.4 [-0.8, +0.1] | 246 of 7,168 |

overall change +0.47 points over 207,872 paired games; significantly better: 15, significantly worse: 0, of 29 decks

15 of the 29 decks got significantly better and none significantly worse.
The knowledge only matters in games where one of its cards shows up, so each
deck changes only 2–8% of its results. Against each other, the new policies
keep their tiers ([DECK-TIER-LIST.md](../DECK-TIER-LIST.md)), except Harpie
Sisters moving from weak to mid: every deck got a little smarter, and the
decks that ran on cards nobody removed (Necrovalley, Toon World) lost the most
ground.

The [summary](card-knowledge-vs-old.summary.json) and
[metadata](card-knowledge-vs-old.metadata.json) record the run. To repeat it,
build the old library from the previous commit and run:

```bash
target/release/policy-bench compare --policies all --opponents all \
  --baseline old/libygo_policies.so --candidate target/release/libygo_policies.so \
  --games 256 --seed 730000 --output knowledge.jsonl
```
