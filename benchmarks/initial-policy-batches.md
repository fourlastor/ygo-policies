# Initial policy batches

These additions implement the remaining engines in [DECK-IDEAS.md](../DECK-IDEAS.md).
The goal is a functional, distinct initial strategy. No policy search is used during initial implementation;
every addition is queued in [search-status.md](search-status.md). Policies use
seat-visible observations and engine-provided legal choices. Engine code and
vendor pointers are unchanged. After the inventory is implemented, a separate
optimization pass is authorized, with all runs capped at **eight workers**.

The first four additions are recorded in [four-new-decks.md](four-new-decks.md).
Subsequent batches below use ordinary matchup duels, alternating seats, 8000 LP,
Master Rule 1 and six workers. Raw duel files stay outside Git. These samples
check operation and give initial strength estimates; different roster sizes
make rates across batches unsuitable for a common ranking.

## Batch 2: Agents, Scrap, Zombies, Herald

40-card Main Decks; Extra Deck sizes 10, 14, 14 and 10 respectively.

- **Heaven's Dispatch** (`agents`): Earth finds Venus/Hyperion, Venus supplies
  bodies, Hyperion removes threats, and Kristya follows available Synchro plays.
  Valhalla deploys the bosses; Orange Light and Honest protect the board.
- **Scrap Renaissance** (`scrap`): Scrapstorm sends Chimera and destroys a Tuner
  for recovery. Chimera and Golem rebuild Synchro materials. Scrap dragons
  spend expendable bodies to remove opposing threats.
- **Graveyard Shift** (`zombie`): recruiters set up Zombie Master, Mezuki and
  Book of Life; Plaguespreader turns revived bodies into Synchros. Discard
  choices favor useful graveyard effects.
- **Herald's Veto** (`herald`): Manju/Senju and Preparation assemble the ritual,
  Dawn/Factory recover Fairy ammunition, and Decree covers traps. The policy
  preserves an established Herald and negates opposing effects.

Shared helpers handle Honest, defensive Stardust effects, conditional Black
Rose wipes, Synchro body/material values, and reachable removal targets.
Shared removal priorities recognize Hyperion, Kristya, Herald and Zombie
revival engines. No opponent hidden identities are inspected.

Validation command (after building `ygo-policies-ffi` and `ygo-policies-bench`):

```sh
target/release/policy-bench matchup --policies agents,scrap,zombie,herald \
  --opponents all --games 16 --seed 830000 --workers 6 \
  --output /tmp/initial-batch2.jsonl
```

**2,400 duels; zero failures and zero decision-limit draws.** Each new policy
played 624 games against the other 39 policies. Workspace tests pass, including
23 focused new-policy tests and pool/script/registry checks.

| Deck | Initial win rate | Wilson 95% interval |
|---|---:|---:|
| Scrap Renaissance | 66.83% | 63.04–70.41% |
| Heaven's Dispatch | 59.62% | 55.72–63.40% |
| Graveyard Shift | 42.47% | 38.65–46.38% |
| Herald's Veto | 32.53% | 28.97–36.31% |

Core effects all activated in the validation: Venus 939, Hyperion 441,
Scrapstorm 500, Chimera 876, Zombie Master 541, Mezuki 279, Herald 1,661,
and Dawn 812. Counts aggregate both seats and include older opponents using
shared cards. [Compact results](initial-batch2.summary.json) and
[run fingerprints](initial-batch2.metadata.json) are retained.

## Batch 3: Fish, Cyber Dragon, Gemini/Plants, Psychics

- **Tidal Assembly** (`fish`): Big Wave Small Wave or tribute bodies deploy
  Coelacanth; its recruitment balances Tuners and Oyster Meister. Fishborg
  follows with Synchros. The deck has 40 Main and 11 Extra cards.
- **Power Surge** (`cyber`): establish Cyber names, fuse with Power Bond or
  Polymerization, fill the graveyard with Future Fusion and Trooper, then
  use Overload. Bond needs a Battle Phase and an LP cushion; Overdragon is
  held over a stronger existing board. 40 Main / 10 Extra.
- **Second Bloom** (`gemini`): Lonefire finds Gigaplant; second summons and
  Supervise enable revival. Chevalier/Spark turn spare cards into removal,
  while Plant Tuners supply Synchros. 40 Main / 13 Extra.
- **Mind Over Matter** (`psychic`): Teleporter recruits a Tuner/non-Tuner pair,
  Jumper makes favorable exchanges, and Psychic Synchros recover LP.
  High-cost effects retain an LP cushion. 40 Main / 14 Extra.

Shared removal priorities now recognize Coelacanth, Gigaplant and Supervise.
Psychic Commander currently uses the adapter's default 100-LP announcement;
its attack estimate and trigger guard use that same 100-point adjustment.

```sh
target/release/policy-bench matchup --policies fish,cyber,gemini,psychic \
  --opponents all --games 16 --seed 840000 --workers 6 \
  --output /tmp/initial-batch3.jsonl
```

**2,656 duels; zero failures and zero decision-limit draws.** Each addition
played 688 games against 43 opponents. Workspace tests pass, including 27
focused new-policy tests. All registered additions are checked for pool and
copy limits, script availability, and correct Main/Extra placement.

| Deck | Initial win rate | Wilson 95% interval |
|---|---:|---:|
| Second Bloom | 59.45% | 55.74–63.05% |
| Tidal Assembly | 50.87% | 47.14–54.59% |
| Mind Over Matter | 40.12% | 36.52–43.82% |
| Power Surge | 23.40% | 20.39–26.71% |

Core activation totals include Coelacanth 544, Power Bond 335, Overload 344,
Gigaplant 1,207, Supervise 956, Teleporter 373, and Jumper 20. Cyber is the
clearest tuning priority in this batch. These are initial results, with no
search gap measured. [Compact results](initial-batch3.summary.json) and
[run fingerprints](initial-batch3.metadata.json) are retained.

## Batch 4: Last Page, Chain Reaction, A Bitter Cure, A Thousand Blades

- **Last Page** (`deckout`): Needle Worm/Jar milling, Book flip cycling and battle shields.
- **Chain Reaction** (`chain-burn`): Different-name draw/burn chains, Chain Strike and Accumulated Fortune.
- **A Bitter Cure** (`nurse`): Resolve Nurse/Simochi before Gift Card and LP-gain burn.
- **A Thousand Blades** (`benkei`): Concentrated equips, Ben Kei multiattacks and Maha Vailo backup.

```sh
target/release/policy-bench matchup --policies deckout,chain-burn,nurse,benkei \
  --opponents all --games 16 --seed 850000 --workers 6 \
  --output /tmp/initial-batch4.jsonl
```

**2,912 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| A Bitter Cure | 752 | 41.16% | 37.69–44.71% |
| Last Page | 752 | 37.10% | 33.72–40.61% |
| A Thousand Blades | 752 | 33.78% | 30.49–37.23% |
| Chain Reaction | 752 | 31.65% | 28.43–35.06% |

Workspace tests pass. [Compact results](initial-batch4.summary.json) and
[run fingerprints](initial-batch4.metadata.json) are retained. No search was
used in this implementation batch.

Batch 4 adds 31-test coverage for all additions. Its dedicated tests cover
Taiyou/Needle Worm, extending one's own burn chain, waiting for Simochi to
resolve, and preserving the Normal Summon before Hidden Armory. Shared
shield logic avoids redundant battle protection within a turn; shared removal
priorities recognize Nurse/Simochi and Ben Kei. Core activations included
Needle Worm 1,063, Jar #2 768, Chain Strike 458, Accumulated Fortune 98,
Simochi 552, Gift Card 454, United We Stand 524, and Mage Power 792.

## Batch 5: Visitors from Beyond, Passing Spirits, The Quiet Grove, Garden of Thorns

- **Visitors from Beyond** (`alien`): Ammonite into Gol'gar, reusable backrow and Code A revival.
- **Passing Spirits** (`spirit`): Kinka revival, returning Creature Swap gifts and tribute Spirits.
- **The Quiet Grove** (`naturia`): Naturia recruitment, Bamboo Shoot tributes and Synchro negation.
- **Garden of Thorns** (`garden`): Plant development, Black Garden tokens and Rose Tentacles attacks.

```sh
target/release/policy-bench matchup --policies alien,spirit,naturia,garden \
  --opponents all --games 16 --seed 860000 --workers 6 \
  --output /tmp/initial-batch5.jsonl
```

**3,168 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| The Quiet Grove | 816 | 57.84% | 54.43–61.19% |
| Visitors from Beyond | 816 | 54.90% | 51.47–58.29% |
| Garden of Thorns | 816 | 45.22% | 41.84–48.65% |
| Passing Spirits | 816 | 28.19% | 25.21–31.37% |

Workspace tests pass. [Compact results](initial-batch5.summary.json) and
[run fingerprints](initial-batch5.metadata.json) are retained. No search was
used in this implementation batch.

Batch 5 passes the 35-test focused suite. New checks cover Naturia-specific
Bamboo tributes, Gol'gar recycling Swords, withholding a harmful Dark Dust
summon, and developing a monster before Black Garden. Shared removal values
recognize Gol'gar, Bamboo Shoot, Code A and Black Garden without claiming
that every revived Bamboo Shoot has its tribute-dependent suppression.
Core activation totals include Ammonite 445, Gol'gar 1,160, Code A 1,745,
Kinka 1,015, Creature Swap 687, Black Garden 4,983 and Rose Tentacles 175.
Bamboo's suppression is continuous, so activation counts do not measure it.

## Batch 6: The Final Sentence, Crown of Venom, Full Charge, Prismatic Forge

- **The Final Sentence** (`destiny-board`): Four opposing End Phases, hand shields and reserved message zones.
- **Crown of Venom** (`venom`): Snake Rain, Vennominon revival and Rise into Vennominaga.
- **Full Charge** (`batteryman`): Micro-Cell/Charger recruitment, AA swarms and Short Circuit.
- **Prismatic Forge** (`gem-knight`): Armadillo materials, recyclable Fusion and Prismaura removal.

```sh
target/release/policy-bench matchup --policies destiny-board,venom,batteryman,gem-knight \
  --opponents all --games 16 --seed 870000 --workers 6 \
  --output /tmp/initial-batch6.jsonl
```

**3,424 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Prismatic Forge | 880 | 49.09% | 45.80–52.39% |
| Full Charge | 880 | 45.80% | 42.53–49.10% |
| The Final Sentence | 880 | 43.30% | 40.06–46.59% |
| Crown of Venom | 880 | 15.80% | 13.54–18.35% |

Workspace tests pass. [Compact results](initial-batch6.summary.json) and
[run fingerprints](initial-batch6.metadata.json) are retained. No search was
used in this implementation batch.

Batch 6 passes the 39-test focused suite. New checks preserve a slot for the
last Spirit Message, stop Snake Rain from milling Vennominaga, prevent
Industrial Strength from destroying our own backrow, and use recoverable
Gem-Knight Fusion as Prismaura's cost. Shared card information adds the
missing Dinosaur/Fish/Sea Serpent/Reptile race constants and removal values
for Destiny Board/messages, Vennominaga and Prismaura.

Core activation totals include Destiny Board 6,682 (including message
placement), Snake Rain 842, Rise 47, Vennominon 218, Vennominaga 74,
Batteryman Charger 514, Short Circuit 274, Inferno Reckless Summon 124,
Gem-Knight Fusion 4,966 (including recovery), and Prismaura 974. Venom's
low win rate despite functional progression makes setup speed a priority
for the subsequent optimization pass.
