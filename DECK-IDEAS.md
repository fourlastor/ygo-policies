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
| `dark-world` | Gates Unopened | Effect discards, early Dark World bosses and Raven Synchros |
| `worm` | Visitors Beneath | Xex/Yagan, repeated Flips and diverse Worm Zero materials |
| `chaos` | Between Light and Dark | LIGHT/DARK trades, Chaos Sorcerer and reactive monster effects |
| `demise` | Armageddon Hour | Demise field wipe, Doom Dozer and Megamorph finishers |
| `amazoness` | Queens of the Wild | Village replacements, Queen protection and reflected battle damage |
| `jurrac` | Footprints in Fire | Battle recruitment, Aeolo revival and Dinosaur Synchros |
| `genex` | Clockwork Current | Undine setup, reusable Normal Summons and Machine Synchros |
| `ice-barrier` | Winter Parliament | Triangle name diversity, Gantala recovery and WATER Synchros |
| `reptilianne` | The Still Gaze | Zero-ATK control, Viper theft, Vaskii tributes and Hydra draws |
| `iron-chain` | Rust Never Sleeps | Repairman/Coil recursion, Synchros and opportunistic milling |
| `malefic` | Eclipse Without End | Supported 4000-ATK summons, Skill Drain and field protection |

Initial implementation used hand-written strategies, ordinary-duel validation
and focused decision tests, without policy search. See
[`benchmarks/four-new-decks.md`](benchmarks/four-new-decks.md) and
[`benchmarks/initial-policy-batches.md`](benchmarks/initial-policy-batches.md) for validation.
The follow-up [optimization pass](benchmarks/new-decks-optimization.md) completed
search diagnostics and independent comparisons for all 39 additions, using at
most eight workers. Twenty-five policies show measured gains; others remain
unchanged or inconclusive. The [current ranking](DECK-TIER-LIST.md) includes all 71 decks.

## Inventory status

All 39 additions in this inventory now have initial decks and policies.

## Coverage boundaries

- Frog Monarch is already covered by *Emperor, Arise!*; Substitoad is forbidden.
- Swarm/Synchro, graveyard combos, Fusion, Ritual, tribute control, trap stun,
  direct attacks, battle positions, equipment, spell counters, coin effects,
  stall, burn, Countdown and Exodia already have representatives.
- No legal Xyz, Pendulum or Link monsters. XYZ-Dragon Cannon is a Fusion.
- T.G. Striker/Warwolf/Hyper Librarian, Reborn Tengu and Rescue Rabbit are
  outside this whitelist. Calendar-era assumptions are insufficient.

The implementation queue and first optimization pass are complete. Remaining
headroom and search coverage are tracked in
[`benchmarks/search-status.md`](benchmarks/search-status.md).
