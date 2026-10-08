# Optimizing the expanded roster

Initial implementation finished at `680d8c2`: 39 new named decks and policies,
added in ten commits, bringing the roster to 71. Lists, engine, card scripts and
card database are fixed during this policy pass. Ordinary policies continue to
use only their seat's observation and legal choices.

All duel/search runs used at most eight workers in
total; the long search was paused while a comparison ran. Large JSONL logs and
replays stay in `/tmp` and are not tracked.

## Method

The initial search uses 52 games per new policy (four per original reference
opponent), seed 920000, strict hidden-monster handling, 4/12/24 sampled worlds,
paired z 1.645, an 8192-decision cap, and the frozen initial library. It is a
small diagnostic search, not a hard ceiling. Search's privileged inventory and
random-state access does not become a policy input.

Development comparisons use the same frozen opponent policies, decks, seats,
shuffle and tie-break seeds for before/after. There are 416 pairs per policy
(32 per reference opponent), seed 930000, with the same 8192 cap. Held-out
confirmation uses different seeds and the original 32-policy opponent pool.

## First candidate

57 focused tests pass. 16,224 paired games (32,448 duels) completed with no
failures or caps. No changed policy lost aggregate win rate in this development
sample. These are development results, not yet held-out evidence.

| Policy | Before | Candidate | Delta (points) |
|---|---:|---:|---:|
| Deckout | 30.05% | 40.87% | +10.82 |
| Amazoness | 45.91% | 55.05% | +9.13 |
| Malefic | 52.40% | 59.62% | +7.21 |
| Nordic | 50.24% | 56.97% | +6.73 |
| Alien | 50.24% | 56.25% | +6.01 |
| Batteryman | 39.66% | 45.67% | +6.01 |
| Zombie | 42.07% | 46.88% | +4.81 |
| Ice Barrier | 31.73% | 36.30% | +4.57 |
| Worm | 32.69% | 37.26% | +4.57 |
| Counter Fairy | 47.12% | 51.20% | +4.09 |
| Ben Kei | 33.65% | 37.26% | +3.61 |
| Chaos | 69.23% | 72.60% | +3.37 |
| Agents | 60.82% | 63.94% | +3.12 |
| Herald | 30.53% | 33.17% | +2.64 |
| Naturia | 51.68% | 53.61% | +1.92 |
| Scrap | 63.22% | 64.42% | +1.20 |
| Iron Chain | 31.01% | 31.25% | +0.24 |

Rules use available Flip Summons, accept legal Honest windows even when the
last published phase is BattleStart, use Fiendish Chain on an attacking monster
or current effect source, enable omitted search/recovery spells, and deploy
otherwise stranded Agents bodies. Original policies do not opt into the new
Flip helper.

## Second candidate

61 focused tests pass; another 16,224 pairs completed with no failures or caps.
Psychic improves from 40.62% to 43.99% after choosing Commander's smallest
winning LP payment (up to 500). Ben Kei reaches 37.74% by summoning its main
attacker before equipping a backup. Chain Burn preserves unique-name chains;
Dark World stores backrow before a Jar flip; Fish uses Snowman and recovers
useful Salvage targets; Reptilianne values Hydra according to visible zero-ATK
targets. These smaller changes still require held-out confirmation. The
Armadillo-before-Fusion experiment was neutral in this sample (13 improved,
13 regressed), and Smoke Ball deployment was nearly neutral (15/14).

## Rejected experiments and further development

A larger 1,664-pair-per-policy trial of Cyber's Overload/Limiter timing,
Spirit's Treeborn/backrow reservation, and Venom's deliberate Damage = Reptile
battle payments was flat: +0.30, -0.18 and +0.06 points. Those rules were
removed. No failures or decision caps occurred.

Candidate 4 completed another 16,224 pairs without failures or caps. It added
independent Trishula/Mist Wurm triggers, Colossal Fighter recovery, Genex's
One for One activation, Demise's Trade-In/Megamorph decisions, and Judgment
protection for Destiny Board's letters. Fabled also sets spare spells before
Ragin refills its hand.

The fifth trial used 832 pairs per selected policy. Drawing with Upstart before
Nurse's conversion engine was ready regressed by 1.92 points against the
initial policy, and forcing Diva ahead of Moray regressed by 0.60 points.
Those rules were removed. Prioritizing Jurrac Synchros before Aeolo was flat
and was also removed. A different Spirit/Venom/Cyber rule is not counted as an
improvement merely because it changes transcripts.

Candidate 6 used the original 416 development pairs per policy. Choosing
Stardust ahead of Red Dragon Archfiend improved Flamvell from 41.35% to 44.71%.
Spore cost selection uses the banished Plant's public Level and available
Synchro levels; Garden reached 43.39% from 40.75%, while Gemini was slightly
negative (59.38% to 58.89%). These remain development findings pending the
larger, fresh-seed comparison. All 16,224 pairs completed without failures or
caps.

Candidate 7 tested the Plant/Fusion rules on 832 pairs per policy. Garden
and Gem-Knight each improved by 2.64 points; Gemini was unchanged (six
improved and six regressed). Copy Plant selects a Level that opens a better
Synchro. Gem-Knight attacks with an established Fusion before fusing it away
on an open opposing field. Garden can attack with small recruiters instead
of always setting them.

The workspace passed all 245 tests, including 66 new-deck tests. An additional
6,448 paired regression games for the original 32 policies produced identical
transcripts in every pair, with no failures or caps. The Commander announcement
hook defaults to the original behavior for all other policies.

A broader validation uses 1,024 pairs per new policy against the original 32
policies, seed 940000. Opponent code stays frozen at the initial library.
This validation is used to reject weak candidates; the final confirmation
uses a separate seed.

Later candidates refined Birdman's bounce target to retain a non-Tuner and
Iron Chain's summon priorities to prefer a live Junk Synchron revival, or
Ryko when the empty field faces a stronger attacker. The broader validation
showed Genex +1.37 points and Iron Chain +5.57 points. Herald's additional
Orange Light/Honest setting experiment was removed because it did not improve
on the earlier Herald candidate.

Gusto's aggressive Creature Swap package was slightly negative in broader
validation (-0.44 points). Removing it and retaining the revival/Gulldos setup
and Egul tribute preservation produced +1.27 points in 2,048 validation pairs.
The initial diagnostic search's 32.7-point gap is much larger; this pass does
not claim to close it.

Jurrac now considers Shrink held in hand when planning a battle against a
reachable face-up attacker, with checks for visible spell locks. Naturia can
keep Antjaw face-up for its opposing-Special-Summon trigger. Development
results were +0.12 and +1.68 points; broader validation was +0.39 and +1.37.
These modest results need the final independent confirmation.

Volcanic's completed search logged 29 missed Raigeki Break activations. The
policy now uses reachable, worthwhile targets and treats Shell/Counter as
discard ammunition. It preserves Scattershot's three-card package rather
than confusing an ordinary discard with an Accelerator wipe. Development
improved from 40.26% to 44.95% (+4.69 points, 832 pairs); broader validation
improved from 42.77% to 50.00% (+7.23, 1,024 pairs), with no failures or caps.

## Final independent comparison

All 39 policies were frozen before the final comparison: seed 950000,
64 games against each of the original 32 opponents, alternating seats,
8192-decision cap and eight workers. There are 79,872 pairs (159,744 duels),
2,048 pairs per policy. These results compare against the initial library
from `680d8c2`, with unchanged lists and frozen initial opponent policies.
All duels completed without failures or decision-limit draws. The average
across these equally weighted policies increased from **44.97% to 47.83%**
(+2.86 points, paired 95% half-width 0.18 points). Twenty-five policies have
individual intervals above zero, ten have no clear gain, and four have
identical transcripts throughout (Cyber, Macro D.D., Nurse and Venom).

The intervals below use the paired score differences; they are approximate,
individual 95% intervals, without a multiple-comparison adjustment.

| Policy | Initial | Final | Delta (points) | Paired 95% half-width | Improved / regressed | Identical transcripts |
|---|---:|---:|---:|---:|---:|---:|
| `deckout` | 37.30% | 49.19% | +11.89 | ±2.19 | 404 / 156 | 504 / 2048 |
| `alien` | 50.83% | 58.35% | +7.52 | ±1.40 | 190 / 36 | 991 / 2048 |
| `volcanic` | 42.19% | 49.41% | +7.23 | ±1.59 | 218 / 70 | 738 / 2048 |
| `batteryman` | 42.04% | 48.73% | +6.69 | ±1.40 | 180 / 43 | 1231 / 2048 |
| `agents` | 58.35% | 64.94% | +6.59 | ±1.65 | 221 / 86 | 670 / 2048 |
| `iron-chain` | 35.45% | 41.94% | +6.49 | ±1.43 | 182 / 49 | 1024 / 2048 |
| `nordic` | 53.08% | 59.47% | +6.40 | ±1.57 | 205 / 74 | 995 / 2048 |
| `malefic` | 50.63% | 56.98% | +6.35 | ±1.25 | 155 / 25 | 1607 / 2048 |
| `ice-barrier` | 36.65% | 42.63% | +5.98 | ±1.21 | 146 / 23 | 1274 / 2048 |
| `amazoness` | 46.14% | 51.86% | +5.71 | ±1.51 | 186 / 69 | 905 / 2048 |
| `worm` | 37.01% | 41.46% | +4.44 | ±1.27 | 136 / 45 | 1384 / 2048 |
| `herald` | 28.37% | 32.76% | +4.39 | ±1.14 | 118 / 28 | 1549 / 2048 |
| `benkei` | 31.30% | 35.11% | +3.81 | ±1.33 | 137 / 59 | 1168 / 2048 |
| `garden` | 41.89% | 45.61% | +3.71 | ±1.26 | 126 / 50 | 948 / 2048 |
| `psychic` | 45.31% | 48.73% | +3.42 | ±0.88 | 78 / 8 | 1625 / 2048 |
| `counter-fairy` | 49.22% | 52.20% | +2.98 | ±0.84 | 70 / 9 | 1744 / 2048 |
| `chaos` | 64.60% | 67.19% | +2.59 | ±0.81 | 63 / 10 | 1771 / 2048 |
| `zombie` | 45.41% | 47.51% | +2.10 | ±1.15 | 94 / 51 | 1556 / 2048 |
| `naturia` | 53.54% | 55.32% | +1.78 | ±1.54 | 148 / 111 | 794 / 2048 |
| `destiny-board` | 49.22% | 50.98% | +1.76 | ±0.59 | 37 / 1 | 1970 / 2048 |
| `flamvell` | 43.02% | 44.68% | +1.66 | ±1.04 | 76 / 42 | 1106 / 2048 |
| `dark-world` | 55.76% | 57.18% | +1.42 | ±1.17 | 90 / 61 | 1285 / 2048 |
| `scrap` | 66.50% | 67.63% | +1.12 | ±0.75 | 42 / 19 | 1771 / 2048 |
| `cloudian` | 48.78% | 49.80% | +1.03 | ±1.15 | 83 / 62 | 1126 / 2048 |
| `gusto` | 50.49% | 51.51% | +1.03 | ±1.10 | 77 / 56 | 1347 / 2048 |
| `fish` | 51.27% | 52.15% | +0.88 | ±0.81 | 45 / 27 | 1732 / 2048 |
| `demise` | 50.49% | 51.27% | +0.78 | ±0.88 | 50 / 34 | 1700 / 2048 |
| `gem-knight` | 44.34% | 45.02% | +0.68 | ±1.17 | 82 / 68 | 1225 / 2048 |
| `chain-burn` | 27.29% | 27.88% | +0.59 | ±0.43 | 16 / 4 | 1830 / 2048 |
| `genex` | 48.78% | 49.22% | +0.44 | ±0.98 | 57 / 48 | 1528 / 2048 |
| `gemini` | 56.98% | 57.18% | +0.20 | ±0.41 | 11 / 7 | 1829 / 2048 |
| `fabled` | 39.70% | 39.79% | +0.10 | ±1.06 | 62 / 60 | 1495 / 2048 |
| `reptilianne` | 44.53% | 44.58% | +0.05 | ±0.82 | 37 / 36 | 1608 / 2048 |
| `cyber` | 23.14% | 23.14% | +0.00 | ±0.00 | 0 / 0 | 2048 / 2048 |
| `macro-dd` | 68.16% | 68.16% | +0.00 | ±0.00 | 0 / 0 | 2048 / 2048 |
| `nurse` | 39.11% | 39.11% | +0.00 | ±0.00 | 0 / 0 | 2048 / 2048 |
| `spirit` | 28.86% | 28.86% | +0.00 | ±0.00 | 0 / 0 | 2040 / 2048 |
| `venom` | 16.06% | 16.06% | +0.00 | ±0.00 | 0 / 0 | 2048 / 2048 |
| `jurrac` | 51.86% | 51.61% | -0.24 | ±0.48 | 10 / 15 | 1911 / 2048 |

Initial search rates for every policy are indexed in [search-status.md](search-status.md).

## Remaining work and ranking

This is a completed first pass, not an exhausted optimization queue. Fabled,
Gusto and Genex did not confirm their development gains on the independent
sample. Jurrac was slightly negative but inconclusive (-0.24 ±0.48 points).
Cyber, Macro D.D., Nurse and Venom retained their initial behavior throughout
the held-out sample; Spirit changed eight transcripts without changing a
result. Useful activation/sequence fixes remain where the interval overlaps
zero, without counting them as measured strength improvements.

Gusto, Genex, Naturia and Jurrac had large initial diagnostic search gaps and
are candidates for a deeper pass. Those gaps were measured before these
changes, on a smaller and different opponent sample; they must not be
subtracted from the held-out gains to estimate a current ceiling. No final
follow-up search was run. Venom's low initial score and small search gap also
make its deck composition a candidate for revision.

The final round robin played **159,040 duels across all 71 decks**, 64 games
per unordered pairing, alternating seats, seed 960000, an 8192-decision cap
and eight workers. It completed with **zero failures and zero decision-limit
draws**. [DECK-TIER-LIST.md](../DECK-TIER-LIST.md) contains the refreshed ranking
and matchup matrix. It uses the existing 13-deck reference pool for tiers;
each deck has 4,480 games against the measured pool. Pool expansion changes
the opponent mix, so the previous 32-deck ranking's rates are not directly
comparable to these rates.

All **251 workspace tests** pass, including 72 new-deck tests covering pool
legality, registry/script availability and strategic decisions. The earlier
6,448-pair original-policy regression check was identical throughout; later
code edits touched only the new policies and their shared helpers. No engine
or vendor changes are part of this work. Policies use their seat's observation
and legal choices; privileged search state is confined to the benchmarking
tool.

[Compact exact results and fingerprints](new-decks-optimization.results.json)
retain seeds, library/deck/database hashes, vendor revisions, initial search
scores and paired before/after scores. Large JSONL/replay files remain outside
Git. The tested final library has SHA-256
`13cf2555da491c3687f432f93cf778b966c6d8d4a21a4f83b83005cba2b539a2`.

To regenerate the ranking after building the release policy library and
benchmark executable:

```sh
target/release/policy-bench round-robin --policies all \
  --library target/release/libygo_policies.so \
  --games 64 --seed 960000 --workers 8 --limit 8192 \
  --output /tmp/new-decks-final-ranking.jsonl --markdown DECK-TIER-LIST.md
```
