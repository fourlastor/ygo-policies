# Initial policy batches

These additions implement the remaining engines in [DECK-IDEAS.md](../DECK-IDEAS.md).
The goal is a functional, distinct initial strategy. No policy search is used during initial implementation;
every addition is queued in [search-status.md](search-status.md). Policies use
seat-visible observations and engine-provided legal choices. Engine code and
vendor pointers are unchanged. After the inventory is implemented, a separate
optimization pass is authorized, with all runs capped at **eight workers**.
The completed pass and independent results are recorded in
[new-decks-optimization.md](new-decks-optimization.md).

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
At initial implementation, Psychic Commander used the adapter's default
100-LP announcement; its attack estimate and trigger guard used that same
100-point adjustment. The subsequent optimization pass adds variable payment.

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

## Batch 7: Ashes to Inferno, Road to Ragnarok, Eye of the Storm, Volcanic Aftershock

- **Ashes to Inferno** (`flamvell`): Firedog recruitment, ordered Laval milling and Rekindling Synchros.
- **Road to Ragnarok** (`nordic`): Hamster/Tanngnjostr recruitment, Thor and Valkyrie into Odin.
- **Eye of the Storm** (`cloudian`): Sanctuary/Barrier defense, Fog Counters and Cloudian removal.
- **Volcanic Aftershock** (`volcanic`): Shell ammunition, Scattershot wipes and Doomfire battle pressure.

```sh
target/release/policy-bench matchup --policies flamvell,nordic,cloudian,volcanic \
  --opponents all --games 16 --seed 880000 --workers 6 \
  --output /tmp/initial-batch7.jsonl
```

**3,680 duels; 0 failures and 2 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Road to Ragnarok | 944 | 54.45% | 51.26–57.60% |
| Eye of the Storm | 944 | 52.33% | 49.14–55.50% |
| Volcanic Aftershock | 944 | 46.29% | 43.13–49.48% |
| Ashes to Inferno | 944 | 44.81% | 41.66–48.00% |

Workspace tests pass. [Compact results](initial-batch7.summary.json) and
[run fingerprints](initial-batch7.metadata.json) are retained. No search was
used in this implementation batch.

Batch 7 passes 43 focused tests. New checks cover Cloudian positions,
Tanngnjostr's recruitment, Laval mill order, and Accelerator's attack cost.
Shared removal values recognize Cloudian Squall and the Accelerators.
Core activation totals include Rekindling 727, Handmaiden 1,034,
Tanngnjostr 1,363, Thor 1,168, Odin 286, Squall 5,665, Altus 2,559,
Blaze Accelerator 2,602, Scattershot 1,243 and Doomfire 192.

Both capped duels were Cloudian–Crystal at the normal 4,096-decision limit.
A separate 16-game rerun of that pair, same seed and seats, with `--limit 8192`
and four workers had zero caps or failures. The two formerly capped games
(engine seeds 128025549 and 128025560) completed after 4,335 and 6,081
choices, on turns 65 and 64, both Cloudian deck-out wins. A recorded replay
confirmed ordinary progress. The original batch table retains the two
capped draws; the higher-limit outcomes are diagnostic, not substituted.

## Batch 8: Gates Unopened, Visitors Beneath, Between Light and Dark, Armageddon Hour

- **Gates Unopened** (`dark-world`): Effect discards, early Dark World bosses and Raven Synchros.
- **Visitors Beneath** (`worm`): Xex/Yagan, repeated Flips and diverse Worm Zero materials.
- **Between Light and Dark** (`chaos`): LIGHT/DARK trades, Chaos Sorcerer and reactive monster effects.
- **Armageddon Hour** (`demise`): Demise field wipe, Doom Dozer and Megamorph finishers.

```sh
target/release/policy-bench matchup --policies dark-world,worm,chaos,demise \
  --opponents all --games 16 --seed 890000 --workers 6 \
  --output /tmp/initial-batch8.jsonl
```

**3,936 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Between Light and Dark | 1008 | 71.23% | 68.36–73.94% |
| Gates Unopened | 1008 | 55.75% | 52.67–58.79% |
| Armageddon Hour | 1008 | 53.77% | 50.68–56.83% |
| Visitors Beneath | 1008 | 38.49% | 35.54–41.53% |

Workspace tests pass. [Compact results](initial-batch8.summary.json) and
[run fingerprints](initial-batch8.metadata.json) are retained. No search was
used in this implementation batch.

Batch 8 passes 47 focused tests. New checks distinguish Dark World effect
discards from costs, prioritize Yagan for Xex, aim Veiler at the current
opposing field effect, and prevent Demise from wiping our own finisher over
an empty opposing field. Worm, Chaos and Dark World explicitly flip their
useful face-down monsters when offered a legal position change.

## Batch 9: Queens of the Wild, Footprints in Fire, Clockwork Current, Winter Parliament

- **Queens of the Wild** (`amazoness`): Village replacements, Queen protection and reflected battle damage.
- **Footprints in Fire** (`jurrac`): Battle recruitment, Aeolo revival and Dinosaur Synchros.
- **Clockwork Current** (`genex`): Undine setup, reusable Normal Summons and Machine Synchros.
- **Winter Parliament** (`ice-barrier`): Triangle name diversity, Gantala recovery and WATER Synchros.

```sh
target/release/policy-bench matchup --policies amazoness,jurrac,genex,ice-barrier \
  --opponents all --games 16 --seed 900000 --workers 6 \
  --output /tmp/initial-batch9.jsonl
```

**4,192 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Footprints in Fire | 1072 | 51.40% | 48.41–54.38% |
| Clockwork Current | 1072 | 50.75% | 47.76–53.73% |
| Queens of the Wild | 1072 | 49.72% | 46.73–52.71% |
| Winter Parliament | 1072 | 38.34% | 35.48–41.29% |

Workspace tests pass. [Compact results](initial-batch9.summary.json) and
[run fingerprints](initial-batch9.metadata.json) are retained. No search was
used in this implementation batch.

Batch 9 passes 51 focused tests. Checks cover Amazoness protection/reflection
and Skill Drain, Meteor's destructive summon, Birdman's required follow-up,
and searching a distinct Ice Barrier name for Triangle. Queen's conditional
protection and Swords Woman's reflection are shared public-board knowledge.

## Batch 10: The Still Gaze, Rust Never Sleeps, Eclipse Without End

- **The Still Gaze** (`reptilianne`): Zero-ATK control, Viper theft, Vaskii tributes and Hydra draws.
- **Rust Never Sleeps** (`iron-chain`): Repairman/Coil recursion, Synchros and opportunistic milling.
- **Eclipse Without End** (`malefic`): Supported 4000-ATK summons, Skill Drain and field protection.

```sh
target/release/policy-bench matchup --policies reptilianne,iron-chain,malefic \
  --opponents all --games 16 --seed 910000 --workers 6 \
  --output /tmp/initial-batch10.jsonl
```

**3,312 duels; 0 failures and 0 decision-limit draws.**

| Deck | Games | Initial win rate | Wilson 95% interval |
|---|---:|---:|---:|
| Eclipse Without End | 1120 | 54.91% | 51.98–57.80% |
| The Still Gaze | 1120 | 44.91% | 42.02–47.84% |
| Rust Never Sleeps | 1120 | 39.64% | 36.82–42.54% |

Workspace tests pass. [Compact results](initial-batch10.summary.json) and
[run fingerprints](initial-batch10.metadata.json) are retained. No search was
used in this implementation batch.

The final implementation batch passes 54 focused tests. Reptilianne spends
opposing zero-ATK monsters before its own, Iron Chain preserves its revival
fuel when a temporary boost is unnecessary, and Malefic requires a supporting
Field Spell or Skill Drain before committing its monsters. All 39 inventory
entries now have named lists and initial policies; the roster contains 71.

The user subsequently authorized optimization after creation. The following
phase uses search, with at most eight execution workers, and keeps these
initial measurements as the before-optimization record.
