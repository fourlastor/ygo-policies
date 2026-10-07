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

These are initial hand-written strategies, with ordinary-duel validation and
focused decision tests. No policy search was used. See
[`benchmarks/four-new-decks.md`](benchmarks/four-new-decks.md) and
[`benchmarks/initial-policy-batches.md`](benchmarks/initial-policy-batches.md) for validation.
The 32-deck tier list predates these additions; they are not ranked there yet.

## Additional win plans and interaction styles

| Plan | Available cards | What to test |
|---|---|---|
| Alien control | Alien Ammonite, Alien Dog, Cosmic Fortress Gol'gar | A-Counters, return face-up Spells/Traps to hand and reuse them. |
| Spirit control | Asura Priest, Hino-Kagu-Tsuchi, Yamata Dragon, Kinka-byo, Izanagi | End Phase returns and repeated Normal Summons. Aratama/Nikitama are unavailable. |
| Naturia | Bamboo Shoot, Cliff, Cherries, Beast, Barkion | Recruiters and Spell/Trap suppression. Generic Naturia Beast in other Extras does not cover the family. |
| Black Garden | Black Garden plus suitable ATK/revival targets | Shared tokens, ATK manipulation, deliberate revival setup; no dedicated list designed yet. |
| Destiny Board | Destiny Board and the Spirit Messages | Backrow-dependent alternate win; lower novelty now that Countdown and Exodia exist. |
| Venom | Venom Swamp, Vennominon, Vennominaga | ATK erosion, a protected boss and an alternate win; demanding setup. |

## Other available families

- **Batteryman:** AA, Charger, Micro-Cell, Industrial Strength, Short Circuit,
  Quick Charger. Swarm into removal and burst damage; different from Watt.
- **Early Gem-Knight:** Fusion, Gem-Armadillo, Alexandrite, Ruby, Citrine.
  Obsidian and Lazuli are outside the pool; do not assume later combo support.
- **Flamvell / early Laval:** Firedog, Magician, Rekindling, Laval Cannon,
  Volcano Handmaiden. Lakeside Lady and Molten Conduction Field unavailable.
- **Nordic/Aesir:** Guldfaxe, Tanngnjostr, Valkyrie, Gleipnir, Odin and Thor.
- **Cloudian:** Turbulence, Smoke Ball, Altus, Acid Cloud, Cirrostratus,
  Cloudian Squall; counters and battle resilience.
- **Volcanic:** Shell, Rocket, Scattershot, Blaze Accelerator, Doomfire.
  Blaze Accelerator Reload unavailable.
- **Early Dark World:** Broww, Goldd, Sillva, Dealings. Grapha, Snoww and
  Gates unavailable; a vanilla Renge in Exodia does not cover this engine.
- **Worm:** Xex, Yagan, King, Zero; W Nebula Meteorite unavailable.
- **Chaos:** Chaos Sorcerer is available; BLS - Envoy is forbidden,
  Lightpulsar and Eclipse Wyvern unavailable.
- **Demise:** Demise and End of the World are available; field wipe into
  battle damage is a separate Ritual direction.
- **Amazoness, Jurrac, Genex, Ice Barrier, Reptilianne, Iron Chain:** legal
  family members exist, but no complete list or strength case has been built.
- **Malefic:** World, Cyber End, Stardust and supporting monsters are present;
  the roster only has a Rainbow Dragon splash. Skill Drain beatdown itself
  is already extensively represented by Verdict/Rock Block.

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
