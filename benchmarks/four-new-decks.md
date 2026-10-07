# Four initial decks and policies

Added on 2026-10-08: Fabled Encore, Heaven's Rebuttal, D.D. Border Patrol,
and Gusto's Reprisal. Each Main Deck has 40 cards, with no Side Deck.
The policies use the ordinary seat observation, public hand counts and
the engine's legal choices. No policy search, hidden-state rollout, or
opponent hidden-card inspection was used.

## Decks and plans

| Policy | Deck | Monsters / Spells / Traps | Extra |
|---|---|---|---:|
| `fabled` | [Fabled Encore](../decks/Fabled%20Encore.ydk) | 22 / 12 / 6 | 15 |
| `counter-fairy` | [Heaven's Rebuttal](../decks/Heaven's%20Rebuttal.ydk) | 14 / 9 / 17 | 0 |
| `macro-dd` | [D.D. Border Patrol](../decks/D.D.%20Border%20Patrol.ydk) | 18 / 11 / 11 | 1 |
| `gusto` | [Gusto's Reprisal](../decks/Gusto's%20Reprisal.ydk) | 18 / 14 / 8 | 15 |

**Fabled Encore** uses Grimro to find missing pieces, Chawa/Nozoochee/Raven
to discard monsters that summon or revive themselves, and Ragin to refill
a small hand. Kushano recovers itself using another discard; Raigeki Break
and Phoenix Wing Wind Blast combine interaction with discard triggers.
Backrow can be set before Ragin's summon. Catsith is not discarded into
removal that would take away its only safe target before its mandatory
trigger resolves. Sending a card to the Graveyard is distinguished from
discarding it, and Macro/Fissure suppress discard-trigger credit.

**Heaven's Rebuttal** protects Bountiful Artemis for draws and Meltiel for
LP/removal, with Zeradias finding Sanctuary. Divine Punishment, Divine Wrath,
Magic Drain and Dark Bribe answer opposing links; shared Solemn/battle-trap
logic handles the rest. Van'Dalgyon supplies a finisher after a successful
Counter Trap. Spare Sanctuary/Zeradias copies pay discard costs. Honest
supports the LIGHT attackers in the Damage Step.

**D.D. Border Patrol** establishes Macro Cosmos, Dimensional Fissure or
Banisher, then reuses Survivor/Scout Plane as Monarch tributes. Allure
prefers to banish Scout Plane: Survivor does not return when banished from
the hand. Caius/Raiza are held when their compulsory effects would hit our
own field. Cyber Dragon is summoned before occupying the field; Fortress
answers opposing Machines. Return from the Different Dimension is reserved
for a meaningful attack push.

**Gusto's Reprisal** starts behind Gulldo/Egul/Winda, and uses Caam + Krebons
or Winda + Mist Valley Soldier to reach Sphreez. Emergency Teleport and
Double Summon help assemble those pairs. With Sphreez, small Gustos attack
stronger visible monsters to reflect damage; recruiters are preferred,
and damage-preventing targets such as Armor Master are avoided. Caam and
Contact recycle recruiters; Limit Reverse and Call revive bodies. Winda's
effect is not assumed to recruit after its own attack. Existing Sphreez is
preserved instead of being spent on a generic Synchro.

## Shared card information

- Public Sphreez grants reflected damage to its controller's face-up Gustos;
  only Sphreez itself is battle-indestructible. Skill Drain disables both.
- All policies account for lethal reflected damage, including piercing
  into a Gusto in Defense Position.
- Unicore's negation knowledge depends on public hand counts being equal.
  The staged probe's result is no longer treated as unconditional negation.
- Gusto recruitment has value when monsters can reach the Graveyard;
  Macro/Fissure remove that credit. D.D. returns gain value under banishment.
- Shared removal priorities now recognize Artemis, Meltiel, Sanctuary,
  Sphreez, Macro and Fissure as engine cards.

## Validation

The initial 256-game smoke test had zero failures and zero decision-limit
draws. Recorded ordinary games exposed the Catsith timing mistake and
Gusto's attacks into damage prevention; focused regression tests cover both.

After those fixes, the complete **4,288-game** matchup run covered every
pair involving at least one of the four new decks: **32 games per pair**,
alternating seats, seed **820000**, six workers, 8000 LP, Master Rule 1.
There were **zero failures and zero decision-limit draws**.

| Deck | Games | Win rate | Wilson 95% interval |
|---|---:|---:|---:|
| D.D. Border Patrol | 1,120 | 69.64% | 66.89–72.26% |
| Heaven's Rebuttal | 1,120 | 51.43% | 48.50–54.35% |
| Gusto's Reprisal | 1,120 | 50.18% | 47.25–53.10% |
| Fabled Encore | 1,120 | 35.36% | 32.61–38.20% |

These are initial strategies and a small per-matchup sample, not optimized
policies or search ceilings. Each new policy faces the other 35 policies;
these rates are not directly comparable with the earlier 32-deck ranking.
Fabled is the clearest candidate for later tuning. The full ranking still
needs a fresh round robin with the expanded roster and shared knowledge.

The workspace tests pass. The new suite has **19 focused tests**, including
pool/copy limits, Main/Extra placement, card-script availability, registry
forking, key policy decisions, hidden-card boundaries and shared interactions.
The EDOPro session test requires permission to bind its local loopback socket.

Reproduce the ordinary duel validation:

```sh
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
target/release/policy-bench matchup \
  --policies fabled,counter-fairy,macro-dd,gusto --opponents all \
  --games 32 --seed 820000 --workers 6 \
  --output /tmp/four-decks-validation.jsonl
cargo test --workspace
```

[Compact results](four-new-decks.summary.json) retain per-opponent rates and
engine activation counts. [Metadata](four-new-decks.metadata.json) retain
library/card/deck fingerprints, vendor revisions and run arguments. Raw duel
records are temporary files and are not included in the repository.

[DECK-IDEAS.md](../DECK-IDEAS.md) records the remaining available engines and
play styles, including pool restrictions and existing partial coverage.
