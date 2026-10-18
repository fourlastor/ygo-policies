# Own-deck LP profile: frozen experiment design

**UNIMPLEMENTED.** This is an opt-in experiment proposal, not an enabled policy,
validated improvement, or claim that the generic policy meets the strength goal.
No games, builds, or tests were run for this design. The accompanying
`deck-profile-values.json` contains a read-only calculation over the current 71
catalog Main Decks using the existing 951-card native metadata audit.

## Motivation and limits

The existing manual two-weight experiment gave Chain Burn 77/256 wins versus
59/256 with defaults on fresh paired seeds. This motivates testing a composition
signal; it does not establish that these weights are optimal, that the result
transfers to another deck, or that evaluation weights are the only weakness.
The separate direct-chain-rule comparison remained 24/128 versus 24/128.

Native DAMAGE categories describe operations, not the affected player or net
benefit. Among the audit's 50 DAMAGE-bearing codes are Lava Golem, Zephyros,
Pashuul, and Psychokinesis. Poison of the Old Man has no initial DAMAGE category:
its target callback assigns the category after selecting the damage option.
Consequently, counting every DAMAGE descriptor would be a poor strategy signal.

The proposed first version recognizes a concentration of spells/traps primarily
devoted to LP operations. It deliberately gives no credit to monster damage,
mixed removal/damage operations, recovery-conversion combinations, or missing
metadata. This is conservative coverage, not a general classifier for every
noncombat win condition.

## Frozen formula

For each copy in the player's original Main Deck, let `U` be the bitwise union of
its initial native categories over descriptors with `effect_type & ACTION != 0`.
Give that copy one credit if it is a Spell or Trap and either:

1. `U` contains DAMAGE and no bits outside `DAMAGE | RECOVER`; or
2. the existing source-backed LP transaction facts explicitly offer damage to
   the opponent. The current facts cover Secret Barrel, Just Desserts, Poison of
   the Old Man, Chain Strike, Magical Explosion, Ookazi, Goblin Thief, and
   Tremendous Fire. This reuses mechanics facts; it is not a deck-name route.

Count each physical copy once, regardless of the number of descriptors. Extra
Deck and Side Deck cards do not enter either count. Use the existing narrow
alternate-art canonicalization; do not reinterpret distant name aliases.

Let `B` be credited copies and `N` the Main Deck size:

```text
alpha = clamp(8 * B / N - 1, 0, 1)
accessible_cards = 0.8 - 0.5 * alpha
life_buffer      = 0.55 + 3.45 * alpha
```

The other eleven coefficients, all feature calculations, native terminal values
of +/-1, and the existing `0.95 * tanh(score / 6)` remain unchanged. For alpha
exactly zero or one, install the literal default or manual endpoint array to
avoid rounding differences. Reject an empty/invalid deck rather than dividing
by zero.

The one-eighth dead zone and one-quarter saturation point are **heuristic,
frozen design choices**, not learned values or established optima. They express
the hypothesis that a few incidental LP cards should not change the objective,
while a quarter of the Main Deck directly devoted to LP operations warrants
testing the previously frozen endpoint. Do not tune these thresholds against
the sentinel results.

Compute this once from the searcher's own original deck and public definitions,
then hold the weights fixed across every proposal, sampled world, validation,
and live decision. Do not use deck names, opponent composition, current hidden
state, live hand contents, or library-specialist decisions. An unknown
composition uses the same arithmetic; unfamiliar or ambiguous card mechanics
fall back conservatively.

## Expected current profiles

The read-only calculation used `card-coverage-v10.json` category unions plus the
current card database. All 71 deck fingerprints and the engine/scripts revisions
match that audit. There are 13 qualifying unique cards under the proposed rule.

| Catalog deck | B / N | Alpha | Accessible cards | Life buffer |
| --- | ---: | ---: | ---: | ---: |
| Chain Burn | 11 / 40 | 1 | 0.3 | 4.0 |
| Burn Princess | 16 / 40 | 1 | 0.3 | 4.0 |
| Fortune Lady | 2 / 40 | 0 | 0.8 | 0.55 |
| Watt | 2 / 40 | 0 | 0.8 | 0.55 |
| Spellcaster | 1 / 40 | 0 | 0.8 | 0.55 |
| Other 66 catalog decks | 0 / 40 | 0 | 0.8 | 0.55 |

Lava Golem receives no numerator credit, and its 3000 ATK is not treated as our
combat capacity. Volcanic monsters and Tri-Blaze Accelerator's mixed
destruction/damage operation receive no credit, so Volcanic remains at defaults.
Dragon, Monarch, Gorz, and Extra Deck damage effects likewise do not route a
battle deck into this profile. Nurse remains at defaults despite its recovery
conversion strategy; that is an explicit limitation.

An ATK-based modifier is intentionally absent: printed ATK alone does not reveal
summon destination, summon feasibility, variable ATK, or the combat value of
recruiter bodies. The proposed density can still overgeneralize to a new hybrid
with many qualifying LP cards, and native categories alone do not prove damage
polarity for every future card. Keep the feature opt-in and report those limits.

## Implementation boundary and provenance

The existing host already extracts only the searcher's own deck definitions in
`engine/fast.rs` before constructing the shared evaluator. That is the natural
one-time integration point. The existing LP facts live in `generic/life.rs`;
expose a narrow read-only mechanics query rather than duplicating its code list
inside the search engine. Do not change any activation or selection policy.

Reject simultaneous manual weight overrides and automatic profiling, and reject
uses where the heuristic cannot affect decisions (plain, rollout-only, or the
existing fully terminal-only case). Missing required profile metadata must not
silently pretend the experiment ran.

Record the profile formula/version, Main Deck fingerprint, B, N, alpha, and all
13 effective named weights per evaluated deck. A multi-deck run cannot truthfully
use one global effective-weight map. Preserve historical metadata as unrecorded;
do not infer a profile retrospectively from a deck name.

## Frozen validation plan

First verify the formula and wiring without changing search work: copy-count and
order invariance; descriptor duplication does not add credits; Extra/Side and
opponent-deck changes have no effect; Poison's known dynamic option is included;
Lava Golem and mixed removal/damage are excluded. Check interpolation at 5, 6, 8,
and 10 qualifying cards in 40, and bit-exact endpoint/default coefficients.
Compare all 71 computed profiles to `deck-profile-values.json`.

Use fresh paired seeds for **Chain Burn and Burn Princess** against their frozen
specialists with the same search/library/budget settings in both arms. Chain
Burn is a transfer/reproduction check; Burn Princess is the new positive
sentinel. Do not reuse the 256 pairs that motivated the design as held-out proof.

Require mechanical default parity for every other catalog deck. Representative
native response/digest checks should cover Volcanic, Draconic Might, Blackwing,
Psychic, Nurse, and Heroes; they exercise hybrid damage, large/variable bodies,
self-damage, conversion, and resource-oriented play. There is no reason to spend
new win-rate samples on an exactly unchanged evaluator unless wiring parity
fails. Keep node/rollout budgets, errors, caps, and paired seeds visible, and make
no speed or strength claim before those measurements.
