# Arcana coin-aware policy validation — 2026-09-30

Branch: `codex/arcana-coin-policy`. Deck lists are unchanged.

The held-out run improves Arcana from **18.33% to 21.88%** against the original
13 policies: **+3.55 percentage points**, or **19.3% more wins**. The paired 95%
interval for the improvement is **+2.77 to +4.32 points**. It remains a weak deck.
The [current full-pool ranking](../DECK-TIER-LIST.md) has also been regenerated
from 103,936 fresh games with the updated policy.

## Changes

- Parse public `MSG_TOSS_COIN` events and attribute them to the resolving chain
  link. Second Coin Toss keeps the favorable result and retries the unfavorable
  one. The Fool prefers tails; other monsters in this list prefer heads.
- Read the public `MSG_CARD_HINT` registrations (heads/tails effect) to remember
  which effect a face-up monster actually has. These survive a control change and
  reset on leaving the monster zone, turning face-down, or removing the hint.
- Set and use Reversal of Fate to repair unfavorable effects. Heads Fool negates
  our targeting effects, so the policy does not waste Reversal on it.
- Set and use Arcana Call to replace a poor effect with a better effect **on the
  same coin side**, with separate recipient/donor selections. Track its copied
  effect and the end-phase restoration from the registration events.
- Set The Chariot without active Second Coin Toss support. A battle flipping it
  face-up does not activate its summon coin, so it can safely switch to attack
  later. Avoid manually Flip Summoning it without retry support.
- Special Summon The Fool and Moon tokens in defense.

Reversal changes a Lua flag label without updating its client hint. A resolved,
non-negated Reversal therefore invalidates the old recorded result: the public
stream alone cannot prove whether immunity prevented the effect. Unknown coins
are not guessed. This deliberately limits follow-up optimization after Reversal.

## Protocol and results

Baseline: repository revision `5a89458e81dd30786fc9be2efa59588ae2661e65`, built as
`/tmp/arcana-baseline.so` before any changes. The candidate is the implementation
on this branch. Library/deck/database fingerprints and dependency revisions are
in [the metadata](arcana-heldout.metadata.json).

Each version played 512 games per opponent on identical fresh seeds, alternating
seats: **6,656 pairs / 13,312 duels**, at 8000 LP under Master Rule 1. Opponents
always used the baseline library. All duels completed: **zero failures, zero
limit draws**. Of the paired results, 471 improved and 235 regressed; 1,885 full
response transcripts remained identical. Draws score half a win.

| Opponent | Before | After | Change (points) |
|---|---:|---:|---:|
| `blackwing` | 6.05% | 7.23% | +1.17 |
| `burn` | 35.35% | 48.44% | +13.09 |
| `crystal` | 44.34% | 47.85% | +3.52 |
| `dragunity` | 7.03% | 8.01% | +0.98 |
| `gishki` | 19.73% | 23.63% | +3.91 |
| `gladiator` | 10.94% | 12.11% | +1.17 |
| `heroes` | 4.30% | 5.66% | +1.37 |
| `infernity` | 9.96% | 15.04% | +5.08 |
| `lightsworn` | 33.98% | 39.65% | +5.66 |
| `monarch` | 8.98% | 9.57% | +0.59 |
| `morphtronic` | 18.55% | 22.46% | +3.91 |
| `rock-block` | 8.40% | 8.98% | +0.59 |
| `spellcaster` | 30.66% | 35.74% | +5.08 |

Reversal activations rose from 0 to 849; Arcana Call activations from 0 to 513.
Chariot effect activations fell from 3,934 to 1,222. These are observed engine
chain activations, not inferred capability counts.

Exploratory runs, not pooled into the held-out result:

- Coin awareness/trap use alone: 1,664 pairs, 18.57% → 19.77% (+1.20 points).
- Adding safer Chariot deployment: 3,328 pairs, 18.30% → 21.21% (+2.91 points).
- The candidate was then frozen and tested on the held-out seeds below.

The full 29-policy round robin places Arcana **26th**, at **1264 Elo**, with
**21.2%** against the original 13 policies and **23.1%** against all 28 opponents.
It used 256 games per matchup, 128 in each seat, with seed base 730000. All
103,936 games completed: zero failures, zero limit draws, and one engine draw.
The paired comparison above isolates the effect of the policy change; the full
tournament measures the current roster's strength on its own seed schedule.
See the [ranking and matrix](../DECK-TIER-LIST.md),
[raw games](round-robin-2026-09-30.jsonl.gz),
[metadata](round-robin-2026-09-30.metadata.json), and
[summary](round-robin-2026-09-30.summary.json).

## Reproduce

Initialize the submodules and build the benchmark and both policy versions as
explained in [the benchmark guide](../crates/ygo-policies-bench/README.md). Then:

```bash
target/release/policy-bench compare \
  --policies arcana --opponents existing \
  --baseline /tmp/arcana-baseline.so --candidate target/release/libygo_policies.so \
  --games 512 --seed 1900000 --workers 6 \
  --output benchmarks/arcana-heldout.jsonl
```

The runner uses the checkout's pinned OCGCore/Lua, CardScripts and BabelCdb by
default. The exact game seeds are in the [compressed raw paired results](arcana-heldout.jsonl.gz);
[the summary](arcana-heldout.summary.json) includes per-matchup paired intervals.
The JSONL was compressed losslessly after the run; decompress it if needed.

## Verification

- Wire-to-policy regressions cover both seats, favorable/unfavorable tosses,
  resolving-link attribution, stale toss expiry, malformed tosses, control
  changes, face-down/reset lifecycles, silent Reversal changes versus negation,
  Fool's targeting restriction, and Arcana Call's two selections/copy/restoration.
- Chariot regression checks Set without retry support and safe repositioning
  after a battle flip.
- The full workspace test suite includes the 26 recorded-duel projection replays
  and the EDOPro loopback server integration test.
- The benchmark reproduced 26/26 baseline-vs-itself trajectories exactly.
  Candidate-vs-baseline checks for **all 13 existing policies** reproduced
  **1,248/1,248** trajectories exactly (8 games per opponent, each policy
  compared separately). See [the parity summary](existing-policies-parity.summary.json).
- All 29 policies completed the full **103,936-game** round robin. Every one of
  the 406 matchups has 256 games and balanced seats. The aggregate scores and
  matrix were checked against the raw games; regenerating the ranking from
  saved results reproduced the generated table and matrix byte for byte.
