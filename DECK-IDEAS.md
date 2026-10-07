# Deck engines to revisit

Inventory checked on 2026-10-08 against `data/wc2011.lflist.conf`, the pinned
`vendor/BabelCdb/cards.cdb`, and the actual registered decklists. The whitelist
has 4,013 legal card names. Availability means an engine can be explored;
it does not establish a legal complete list or measured strength by itself.
Use this file as a backlog, and recheck the pool when changing vendors.

## Implemented from this inventory

| Policy | Deck | Initial plan |
|---|---|---|
| `fabled` | Fabled Encore | Discard-triggered bodies, Grimro searches, Ragin refills, Synchro finishers |
| `counter-fairy` | Heaven's Rebuttal | Artemis draws, Sanctuary/Meltiel removal, Counter Traps, Van'Dalgyon |
| `macro-dd` | D.D. Border Patrol | Macro/Fissure, recurring Survivor/Scout Plane, Monarch tributes |
| `gusto` | Gusto's Reprisal | Recruiter defense, Sphreez damage reflection, Caam/Contact recycling |
| `agents` | Heaven's Dispatch | Earth/Venus Fairy swarm, Hyperion removal and Kristya |
| `scrap` | Scrap Renaissance | Scrapstorm setup, Chimera/Golem recovery, Scrap Dragon removal |
| `zombie` | Graveyard Shift | Recruiters, Zombie Master/Mezuki revival and Plaguespreader Synchros |
| `herald` | Herald's Veto | Herald ritual control, Fairy recovery, Kristya and Royal Decree |
| `fish` | Tidal Assembly | Coelacanth recruitment, Oyster tokens and Fish Synchros |
| `cyber` | Power Surge | Cyber names, Power Bond, Future Fusion and Overload finishers |
| `gemini` | Second Bloom | Lonefire/Gigaplant, Supervise revival and Gemini Spark removal |
| `psychic` | Mind Over Matter | Teleporter Synchros, Psychic Jumper exchanges and LP recovery |
| `deckout` | Last Page | Needle Worm/Jar milling, Book flip cycling and battle shields |
| `chain-burn` | Chain Reaction | Different-name draw/burn chains, Chain Strike and Accumulated Fortune |
| `nurse` | A Bitter Cure | Resolve Nurse/Simochi before Gift Card and LP-gain burn |
| `benkei` | A Thousand Blades | Concentrated equips, Ben Kei multiattacks and Maha Vailo backup |
| `alien` | Visitors from Beyond | Ammonite into Gol'gar, reusable backrow and Code A revival |
| `spirit` | Passing Spirits | Kinka revival, returning Creature Swap gifts and tribute Spirits |
| `naturia` | The Quiet Grove | Naturia recruitment, Bamboo Shoot tributes and Synchro negation |
| `garden` | Garden of Thorns | Plant development, Black Garden tokens and Rose Tentacles attacks |
| `destiny-board` | The Final Sentence | Four opposing End Phases, hand shields and reserved message zones |
| `venom` | Crown of Venom | Snake Rain, Vennominon revival and Rise into Vennominaga |
| `batteryman` | Full Charge | Micro-Cell/Charger recruitment, AA swarms and Short Circuit |
| `gem-knight` | Prismatic Forge | Armadillo materials, recyclable Fusion and Prismaura removal |
| `flamvell` | Ashes to Inferno | Firedog recruitment, ordered Laval milling and Rekindling Synchros |
| `nordic` | Road to Ragnarok | Hamster/Tanngnjostr recruitment, Thor and Valkyrie into Odin |
| `cloudian` | Eye of the Storm | Sanctuary/Barrier defense, Fog Counters and Cloudian removal |
| `volcanic` | Volcanic Aftershock | Shell ammunition, Scattershot wipes and Doomfire battle pressure |

These are initial hand-written strategies, with ordinary-duel validation and
focused decision tests. No policy search was used. See
[`benchmarks/four-new-decks.md`](benchmarks/four-new-decks.md) and
[`benchmarks/initial-policy-batches.md`](benchmarks/initial-policy-batches.md) for validation.
The 32-deck tier list predates these additions; they are not ranked there yet.

## Other available families

| Engine | Available building blocks / caveat |
|---|---|
| Early Dark World | Broww, Goldd, Sillva, Dealings; Grapha, Snoww and Gates unavailable |
| Worm | Xex, Yagan, King, Zero; W Nebula Meteorite unavailable |
| Chaos | Chaos Sorcerer; BLS Envoy forbidden, Lightpulsar and Eclipse Wyvern unavailable |
| Demise | Demise and End of the World; field wipe into battle damage |
| Amazoness | Legal family members available; initial list pending |
| Jurrac | Legal family members available; initial list pending |
| Genex | Legal family members available; initial list pending |
| Ice Barrier | Legal family members available; initial list pending |
| Reptilianne | Legal family members available; initial list pending |
| Iron Chain | Legal family members available; initial list pending |
| Malefic | World, Cyber End, Stardust; existing roster only has a Rainbow splash |

## Coverage boundaries

- Frog Monarch is already covered by *Emperor, Arise!*; Substitoad is forbidden.
- Swarm/Synchro, graveyard combos, Fusion, Ritual, tribute control, trap stun,
  direct attacks, battle positions, equipment, spell counters, coin effects,
  stall, burn, Countdown and Exodia already have representatives.
- No legal Xyz, Pendulum or Link monsters. XYZ-Dragon Cannon is a Fusion.
- T.G. Striker/Warwolf/Hyper Librarian, Reborn Tengu and Rescue Rabbit are
  outside this whitelist. Calendar-era assumptions are insufficient.

The remaining entries are the implementation queue. Additions use initial
hand-written policies, with search deferred to `benchmarks/search-status.md`.
