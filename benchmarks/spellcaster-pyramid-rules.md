# Spellcaster's Command and Pyramid of Light

These were the weakest remaining eligible entries in
[search-status.md](search-status.md): Spellcaster was 27th and Pyramid 25th.
Baseline: `13562caba6c0ed2670448f64594199a15d48df0f`. The candidate changes only these two
policies on `codex/spellcaster-pyramid-policies`; the deck lists and other
policies are unchanged. Harpie remains in the measured pool, with policy work
deferred until its illegal list is replaced and remeasured.

## Held-out validation

Seed `15830000` was reserved until rule selection was complete. Each version
played 512 games against each of all 30 other decks. Both versions face the
**baseline opponent**, with identical shuffles, seats and policy randomness.
No tuning used these held-out results. Draws count as half a win.

| Pilot | Games per version | Before | After | Gain, percentage points [paired 95% CI] |
|---|---:|---:|---:|---:|
| Spellcaster's Command | 15,360 | 26.14% | 30.67% | +4.52 [+4.05, +5.00] |
| Pyramid of Light | 15,360 | 28.35% | 31.74% | +3.39 [+2.92, +3.86] |

There were **zero failed games and zero decision-limit draws**. Intervals are
paired normal approximations. These measure average performance across the
roster; they do not guarantee an improvement in every matchup. Per-matchup
results are included in the summary linked below. Spellcaster improved against
29 opponents and declined by 0.20 points against Countdown. Pyramid improved
against 28, tied Countdown, and declined by 0.39 points against Karakuri.
These individual matchups have smaller samples than the aggregate.

## Rules retained

Spellcaster:

- Deliberately Flip Summon Crystal Seer for its draw, Old Vindictive Magician
  when an opposing monster can be destroyed, and Mandragola when a counter
  collector is present. Generic repositioning excludes Flip monsters, so
  these effects previously depended on the opponent attacking them.
- Equip an available attacker even when it already beats the opposing ATK,
  including direct attacks. Previously, equips were used only to cross an
  opposing ATK threshold. Mage Power now counts its actual 500 ATK per own
  Spell/Trap, including the field zone and the incoming equip. A set Mage
  Power already occupies a zone and is counted only once.
- Use Magicians Unite when its 3000 ATK breaks a visible ATK wall, or when
  one 3000-ATK direct attack exceeds the combined available Spellcaster damage.
  Preserve multiple attacks when they already deal more damage.

Pyramid:

- Preserve Guardian Sphinx and Des Lacooda's available attacks before turning
  them face-down again. Open Main Phase chain windows omit the Enter Battle
  option; the old rule mistook that absence for battle being unavailable and
  reset Guardian Sphinx before it could attack. Main Phase 2 still resets them.
- Have Pyramid Turtle recruit Spirit Reaper in Defense Position against
  opposing ATK above 1800 during the opponent's turn. Reaper survives battles
  that would destroy the previously preferred Regenerating Mummy.
- Avoid tributing a valuable Sphinx for Guardian Sphinx or Hieracosphinx when
  no lower-value tribute is available. A small body still enables the summon.
- Use Pyramid Energy's +200 ATK to cross a visible battle threshold or finish
  an open field. Keep the existing +500 DEF response to save a defender;
  the option prompt now chooses ATK for our attacks and DEF on opposing turns.

All rules use seat-visible observations, our own cards, and legal choices.
They do not read engine snapshots, opposing hidden cards, deck order, or future
random outcomes. No shared projection or decision API changed.

## Development measurements

Screening used seed `14720000`, 256 games per reference opponent: 3,072 paired
Spellcaster games and 3,328 Pyramid games per candidate. Spellcaster belongs
to the 13-policy reference pool, so its self-match is excluded.

| Independent candidate | Screening gain |
|---|---:|
| Spellcaster: deliberate Flip summons | +2.12 pp |
| Spellcaster: equip winning/direct attackers | +2.16 pp |
| Spellcaster: Magicians Unite for attacks | +0.36 pp |
| Pyramid: preserve the Flip monsters’ attacks | +3.22 pp |
| Pyramid: recruit Reaper against larger attackers | +1.02 pp |
| Pyramid: avoid expensive tributes | +0.78 pp |
| Pyramid: offensive Energy | +0.36 pp |

Counting Mage Power's actual boost added +0.26 points over the combined Flip
and equip rules in its screen. The final policies were checked at 1,024 games
per reference opponent: Spellcaster **+4.24 [+3.73, +4.75] pp** over
12,288 pairs; Pyramid **+4.99 [+4.47, +5.51] pp** over
13,312 pairs.

A separate larger comparison against the simpler combined rules measured
Spellcaster's Mage Power calculation plus Unite at **+0.51 [+0.34, +0.68]
pp**, and Pyramid's offensive Energy at **+0.21 [+0.02, +0.39] pp**.
Those additions were retained. Screening and development intervals are
exploratory; the fresh held-out comparison is the confirmation.

Other ideas were discarded: Apprentice's recruitment chain handling,
choosing Exemplar's level by card value, declining Nuzzler's graveyard recycle,
using Monk with just one Spell in hand, earlier defensive Sets, summoning
Guardian face-up, lowering the Sphinx LP reserve, and early Reaper attacks.
They showed no clear improvement or lost points. Summoning counter collectors
before Spells added only +0.33 points over the simpler combination with an
interval crossing zero; it was also left out. The unused experimental
announcement hook was removed.

## Search headroom

Before and after searches used seed `13610000`, eight games per reference
opponent, and the default 8/32/96-world stages, with strict mode enabled and
foresight disabled. Search logs exposed missed Spellcaster Flip summons and
Unite activations, premature Guardian Sphinx resets, and unused Pyramid Energy.
Recruitment and tribute rules also came from inspecting the policy and card
scripts; search does not explore card-selection alternatives.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Spellcaster's Command | before | 96 | 27.1% | 37.5% | 10.4 pp |
| Spellcaster's Command | after | 96 | 29.2% | 37.5% | 8.3 pp |
| Pyramid of Light | before | 104 | 33.7% | 36.5% | 2.9 pp |
| Pyramid of Light | after | 104 | 33.7% | 41.3% | 7.7 pp |

These small discovery samples estimate headroom, not hard ceilings or gains
guaranteed achievable by an information-limited policy. Strict search skips
positions with hidden opposing monsters, but samples from the opponent's real
deck composition and preserves engine random state, including future coin and
die outcomes. Those privileges belong to development search, not the ordinary
policies or held-out comparisons. Search is one step on top of the current
pilot. The follow-up Pyramid search also faces the improved Spellcaster
reference opponent; the paired comparisons above keep opponents fixed.
No search playout failed and no searched duel hit the decision limit.

## Ranking refresh and validation

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from the complete
119,040-game round robin: all 31 decks, 256 games per pair, seed `730000`.
It had zero failures and zero decision-limit draws.

| Deck | Rank before → after | Measured-pool score before → after |
|---|---:|---:|
| Spellcaster's Command | #27 → #26 | 25.7% → 29.8% |
| Pyramid of Light | #25 → #24 | 27.7% → 31.7% |

All **103,936 games involving neither changed policy** reproduced the previous
tournament's outcomes and transcript digests exactly. Rebuilding the tier list
from saved results reproduced its Markdown byte-for-byte.

**95 workspace tests passed**, including eight new regression tests for
Flip effects, equip timing and Mage Power's zone count, Unite's attack tradeoff,
Sphinx reset timing, Reaper recruitment and position, tribute preservation,
and Energy's offensive and defensive options.

- [Paired, screening and search results](spellcaster-pyramid-2026-10-06.summary.json)
- [Run metadata and raw-result hashes](spellcaster-pyramid-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-spellcaster-pyramid.summary.json)
- [Ranking metadata](round-robin-2026-10-06-spellcaster-pyramid.metadata.json)

## Reproduction

Preserve the baseline shared library at the revision above, then build the
candidate with `cargo build --release -p ygo-policies-ffi -p ygo-policies-bench`.
With `$BASELINE` and `$CANDIDATE` naming the libraries and `$OUT` a fresh directory:

```sh
target/release/policy-bench compare \
  --policies spellcaster,pyramid --opponents all --games 512 --seed 15830000 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" --workers 12 \
  --output "$OUT/heldout.jsonl"

# Repeat with each library for before/after search headroom.
target/release/policy-bench search \
  --policies spellcaster,pyramid --opponents existing --games 8 --seed 13610000 \
  --library "$CANDIDATE" --workers 12 --strict true --foresight false \
  --worlds 8 --confirm 32 --final 96 --log true --record true \
  --output "$OUT/search.jsonl"

target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 12 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```

Engine, scripts, database and deck identities are recorded in metadata.
Large JSONL logs, recorded positions and frozen libraries remain local in
`/tmp/ygo-spellcaster-pyramid.kvc80fnb`; they are not added to Git.
