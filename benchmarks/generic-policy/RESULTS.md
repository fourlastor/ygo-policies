# Generic policy evaluation — 2026-10-14

**The requested >=55% against each specialist was not achieved.** This is an
experimental deck-independent pilot and search implementation, not a qualified
replacement for the specialist policies. Branch: `codex/fast-generic-policy`;
implementation commit: `3da4498`; frozen specialist baseline: `7ddad06`.

## Final evidence

Both players used the same deck. Seeds and seats match between each searched run
and its plain control. These final runs share identical candidate executable and
library fingerprints. All scheduled games completed with **zero game errors,
zero decision-limit caps, and zero search/simulation failures**.

| Configuration | Scope | Wins / games | Win rate | Required matchups with observed >=55% |
|---|---|---:|---:|---:|
| Plain generic | All 71 decks | 181 / 568 | 31.87% | 10 / 70 |
| Search, 200 ms soft budget | All 71 decks | 179 / 568 | 31.51% | 10 / 70 |
| Plain control for larger search | Four decks | 10 / 32 | 31.25% | 0 / 4 |
| Search, up to 256 rollouts, no time limit | Four decks | 14 / 32 | 43.75% | 0 / 4 |

Countdown is the only exception, and remains in aggregate results and the table
below. No required matchup had a Wilson 95% lower bound at or above 55% in either
full-roster configuration. There are only eight games per deck: this is a broad
screen, not sufficient evidence to certify a 55% policy. The result nevertheless
fails the requested observed threshold across most matchups.

The 200 ms search changed outcomes in both directions: 73 improved pairs, 75
regressed, 420 unchanged. Its paired gain was **-0.35 percentage
points**, approximate 95% interval **[-4.55,
+3.85]**. It did not demonstrate an improvement.
The unrestricted four-deck test gained **+12.50 points**, interval
**[-6.68, +31.68]**;
32 pairs are too few to establish that gain. It won 4/8 Blackwing, 2/8 HERO,
4/8 Machina and 4/8 Monarch mirrors. Neither configuration meets the strength goal.

The validated comparisons are [full roster](final-comparison.json) and
[larger search](final-strength-comparison.json). Raw games, summaries and input
fingerprints are stored alongside them. `analyze.py` checks exact schedules,
raw failures, cached summary consistency and fixed inputs; it rejects runs marked
invalid by adjacent `.status.json` files.

## Search and timing

The full-roster search used 8 worlds, at most 8 candidate responses, at most 64
rollouts, full-duel continuations and a 200 ms soft budget. It attempted 266,195
rollouts over 22,575 searched decisions. The search discarded 18,550 incomplete worlds and
reported one unresolved rollout; wall-budget exhaustion occurred 18,554 times. Only complete paired worlds affect
a choice. A large budget does not compensate for inaccurate generic continuations.

The unrestricted search used 32 worlds, at most 8 responses and 256 rollouts,
full-duel continuations and no time budget. It attempted 134,368 rollouts;
138 unresolved rollouts caused 84 incomplete worlds to be discarded. Capped
simulations are never treated as draws or wins. Depth-two search is implemented
and passes replay/budget tests, but is not covered by these final strength runs.

Latency is policy plus search time, excluding surrounding live engine processing.
The first timing column includes forced/simple prompts; the second isolates
prompts that actually ran search. Units are **milliseconds**, shown as p50 / p95 /
maximum.

| Configuration | All own decisions | Searched decisions only |
|---|---:|---:|
| Plain generic | 0.002 / 0.027 / 6.040 | n/a |
| 200 ms search | 0.117 / 203.072 / 332.639 | 201.752 / 208.402 / 332.639 |
| Unrestricted search | 0.107 / 2293.883 / 15019.758 | 1302.095 / 7112.841 / 15019.758 |

These runs overlapped on an Intel Core Ultra 9 285K, 24 logical CPUs: 12 full-roster
search workers and 8 unrestricted-search workers, with small control/test jobs.
See [hardware and load context](hardware.json). The 200 ms target is **not a hard
latency guarantee**: engine/snapshot operations cannot be interrupted, and the
measured searched-decision p95 was 208.4 ms, maximum 332.6 ms under that load.
Wall-clock budgets can change choices with machine load.

A separate one-worker timing smoke, after the large jobs finished, used a 180 ms
budget on eight games across Blackwing/HERO/Machina/Monarch (seed 2443000):
1,497 own decisions, 172 searched, searched p95 **181.28 ms**, maximum **184.72 ms**.
All games completed with zero errors/caps/search failures. This small run checks
latency only; its 2/8 wins are not a strength estimate or a matched comparison.
See [timing smoke summary](final-latency.summary.json).

## Generality and integration

The pilot takes any caller-provided deck and does not identify one of the 71
catalog decks or invoke their specialists. Shared staple behavior, public board
information, printed card properties and legal prompts are reusable inputs.
Synthetic unseen-card tests and an [unregistered-deck integration run](final-custom-deck-smoke.json)
exercise that interface; neither proves strength on arbitrary unseen decks.

The plain `generic` policy works through the ordinary Rust/FFI interface without
an opponent deck list. Search is an engine-hosted API (`engine::play_fast`), not a
hidden feature of the ordinary FFI choice call. Its initial implementation assumes
**known opponent deck composition**, samples unknown arrangements and future RNG,
and uses generic policies for both simulated seats. It skips live roots with an
opponent facedown monster, an active chain, or an unsafe hidden-card selection.
Unsupported hidden backrow placements are rejected. Those are material limits on
its coverage and strength. See the [implementation/use guide](README.md).

## Verification and diagnosis

- `cargo test --workspace`: 281 passed, 0 failed, 1 ignored release-dependent test.
- That release-dependent integration test was then run explicitly and passed:
  deterministic fixed-work choices, depth-two branching, combined rollout budget,
  live-state restore and replay to the same winner.
- Final specialist regression: all 1,820 paired response transcripts were identical,
  with zero errors or caps. [Summary](final-specialist-regression.summary.json).
- The unregistered 40-card deck example completed with 224 rollouts, zero failed rollouts,
  and no cap. This is an integration smoke, not a deck-legality/strength test.
- Field sampling tests check every intermediate exchange, Field/ordinary-zone
  separation, inventory preservation and independence from actual hidden ordering.
- Generic summon tests use unseen IDs and prevent printed-level assumptions from
  vetoing engine-legal alternate summon procedures.

[Recorded failures](DIAGNOSTICS.md) show the remaining problem: generic
continuations spend fusion resources without improving the board, order setup
cards after their payoff window, and value graveyard destinations poorly. The
specific legal-summon veto in those earlier traces is fixed. Better effect and
whole-transaction planning is the next research direction; success is not
established by these experiments, and simply raising the rollout budget did not
reach the target.

## Earlier runs and validity

`plain-screen`, `heldout-plain`, and `plain-smoke-v2` measure earlier plain-pilot
revisions and are not final-policy results. All pre-fix searched smoke runs and
the interrupted `heldout-fast`/`heldout-depth2` runs are explicitly marked invalid
diagnostics: review found their sampler allowed Field Spells in ordinary set
backrow and ordinary cards in the Field Zone. `search-smoke-v1` additionally
failed every rollout due to a pending-response bug. Their raw evidence is retained
for debugging only. A historical `summary.complete` means mechanical completion,
not semantic validity; `.status.json` takes precedence. No invalid run contributes
to the results above.

## Per-deck final screen

Eight games each; entries show wins/8 and raw win rate. Draws are not wins.
The per-deck Wilson intervals are in the adjacent summaries; these are individual,
not simultaneous, confidence intervals across the roster.

| Mirror deck | Plain generic | 200 ms search | Search observed >=55% |
|---|---:|---:|:---:|
| agents | 5/8 (62.50%) | 3/8 (37.50%) | no |
| alien | 2/8 (25.00%) | 5/8 (62.50%) | yes |
| amazoness | 6/8 (75.00%) | 5/8 (62.50%) | yes |
| arcana | 2/8 (25.00%) | 3/8 (37.50%) | no |
| batteryman | 1/8 (12.50%) | 2/8 (25.00%) | no |
| benkei | 3/8 (37.50%) | 4/8 (50.00%) | no |
| blackwing | 7/8 (87.50%) | 4/8 (50.00%) | no |
| burn | 4/8 (50.00%) | 2/8 (25.00%) | no |
| chain-burn | 0/8 (0.00%) | 2/8 (25.00%) | no |
| chaos | 2/8 (25.00%) | 3/8 (37.50%) | no |
| cloudian | 1/8 (12.50%) | 3/8 (37.50%) | no |
| countdown (exception) | 4/8 (50.00%) | 3/8 (37.50%) | no |
| counter-fairy | 3/8 (37.50%) | 4/8 (50.00%) | no |
| crystal | 4/8 (50.00%) | 2/8 (25.00%) | no |
| cyber | 2/8 (25.00%) | 4/8 (50.00%) | no |
| dark-world | 4/8 (50.00%) | 5/8 (62.50%) | yes |
| deckout | 1/8 (12.50%) | 1/8 (12.50%) | no |
| demise | 0/8 (0.00%) | 2/8 (25.00%) | no |
| destiny-board | 0/8 (0.00%) | 0/8 (0.00%) | no |
| destiny-hero | 3/8 (37.50%) | 3/8 (37.50%) | no |
| draconic-might | 3/8 (37.50%) | 2/8 (25.00%) | no |
| dragunity | 1/8 (12.50%) | 1/8 (12.50%) | no |
| exodia | 0/8 (0.00%) | 0/8 (0.00%) | no |
| fabled | 2/8 (25.00%) | 3/8 (37.50%) | no |
| fish | 0/8 (0.00%) | 0/8 (0.00%) | no |
| flamvell | 2/8 (25.00%) | 4/8 (50.00%) | no |
| fortune-lady | 0/8 (0.00%) | 3/8 (37.50%) | no |
| garden | 2/8 (25.00%) | 1/8 (12.50%) | no |
| gem-knight | 2/8 (25.00%) | 0/8 (0.00%) | no |
| gemini | 1/8 (12.50%) | 2/8 (25.00%) | no |
| genex | 3/8 (37.50%) | 1/8 (12.50%) | no |
| gishki | 4/8 (50.00%) | 3/8 (37.50%) | no |
| gladiator | 1/8 (12.50%) | 1/8 (12.50%) | no |
| gravekeeper | 3/8 (37.50%) | 2/8 (25.00%) | no |
| gusto | 2/8 (25.00%) | 2/8 (25.00%) | no |
| harpie | 3/8 (37.50%) | 2/8 (25.00%) | no |
| herald | 6/8 (75.00%) | 4/8 (50.00%) | no |
| heroes | 5/8 (62.50%) | 2/8 (25.00%) | no |
| ice-barrier | 5/8 (62.50%) | 2/8 (25.00%) | no |
| infernity | 1/8 (12.50%) | 0/8 (0.00%) | no |
| iron-chain | 3/8 (37.50%) | 2/8 (25.00%) | no |
| jurrac | 0/8 (0.00%) | 0/8 (0.00%) | no |
| karakuri | 2/8 (25.00%) | 1/8 (12.50%) | no |
| lightsworn | 2/8 (25.00%) | 4/8 (50.00%) | no |
| machina | 1/8 (12.50%) | 2/8 (25.00%) | no |
| macro-dd | 7/8 (87.50%) | 7/8 (87.50%) | yes |
| malefic | 7/8 (87.50%) | 7/8 (87.50%) | yes |
| monarch | 1/8 (12.50%) | 1/8 (12.50%) | no |
| morphtronic | 3/8 (37.50%) | 2/8 (25.00%) | no |
| naturia | 6/8 (75.00%) | 7/8 (87.50%) | yes |
| nordic | 2/8 (25.00%) | 2/8 (25.00%) | no |
| nurse | 0/8 (0.00%) | 2/8 (25.00%) | no |
| ojama | 5/8 (62.50%) | 5/8 (62.50%) | yes |
| psychic | 2/8 (25.00%) | 2/8 (25.00%) | no |
| pyramid | 2/8 (25.00%) | 0/8 (0.00%) | no |
| quickdraw-plant | 3/8 (37.50%) | 3/8 (37.50%) | no |
| reptilianne | 1/8 (12.50%) | 1/8 (12.50%) | no |
| rock-block | 3/8 (37.50%) | 1/8 (12.50%) | no |
| scrap | 3/8 (37.50%) | 2/8 (25.00%) | no |
| six-samurai | 1/8 (12.50%) | 1/8 (12.50%) | no |
| spellcaster | 0/8 (0.00%) | 3/8 (37.50%) | no |
| spirit | 3/8 (37.50%) | 2/8 (25.00%) | no |
| tele-dad | 4/8 (50.00%) | 2/8 (25.00%) | no |
| toon | 3/8 (37.50%) | 1/8 (12.50%) | no |
| venom | 3/8 (37.50%) | 6/8 (75.00%) | yes |
| verdict | 4/8 (50.00%) | 5/8 (62.50%) | yes |
| volcanic | 3/8 (37.50%) | 3/8 (37.50%) | no |
| watt | 2/8 (25.00%) | 2/8 (25.00%) | no |
| worm | 1/8 (12.50%) | 2/8 (25.00%) | no |
| x-saber | 0/8 (0.00%) | 1/8 (12.50%) | no |
| zombie | 4/8 (50.00%) | 5/8 (62.50%) | yes |
