# Initial policy batches

These additions implement the remaining engines in [DECK-IDEAS.md](../DECK-IDEAS.md).
The goal is a functional, distinct initial strategy. No policy search is used;
every addition is queued in [search-status.md](search-status.md). Policies use
seat-visible observations and engine-provided legal choices. Engine code and
vendor pointers are unchanged.

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
