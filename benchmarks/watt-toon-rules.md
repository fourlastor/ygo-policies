# Watt and Toon policies

Watt Grid and Toon Kingdom were the weakest remaining eligible decks after
the Ojama/Destiny HERO round. The deck lists and ranking pool stay fixed,
including the unchanged Harpie Sisters list awaiting correction.

## Held-out validation

The baseline includes the validated [Ojama/Destiny HERO changes](ojama-destiny-rules.md)
on top of `bc8c0f76c349df214ddf4cbfa07a1860e85a8a63`. Both versions face the
baseline opponent on identical shuffles, seats and random seeds. Each pilot
plays 512 games against all 30 other policies, seed `6730000`.

| Pilot | Games per version | Before | After | Change, percentage points [95% CI] |
|---|---:|---:|---:|---:|
| Watt Grid | 15,360 | 17.52% | 20.49% | +2.97 [+2.55, +3.39] |
| Toon Kingdom | 15,360 | 20.65% | 24.07% | +3.42 [+3.00, +3.84] |

Intervals use paired normal approximations; draws count as half a win.
There were no failed games or decision-limit draws. These are average gains
across the roster, not guarantees for individual matchups. Both decks
remain weak. The held-out results were not used for further tuning.

## Rules retained

Watt:

- Use Wattcube's ignition effect before battle to replace a bonus below
  1000 ATK with its lasting +1000 ATK modifier. The old policy equipped it
  but never used this effect. Keep the equip once ten Thunder monsters in
  the Graveyard already supply at least that much ATK.
- Prefer a legal direct attack and its damage-triggered effect to attacking
  an opposing monster. Clear the target intent so the following direct-attack
  yes/no prompt follows that plan.

Toon:

- Toon Table of Contents searches Toon World when missing, then favors
  playable monsters. Tribute monsters get priority only with sufficiently
  cheap material already available. Otherwise Gemini Elf and Masked Sorcerer
  come before expensive bodies; redundant Worlds and duplicate hand cards
  are deprioritized.
- Put Toon Defense up while there is a small face-up Toon to protect, before
  the opponent declares an attack. Apply the same redirection safety check
  to both chain and yes/no prompts. Previously the yes/no prompt defaulted
  to yes, even when converting an attack into direct damage was lethal or
  sacrificed a battle the Toon would win.

The search logs showed unused Wattcube effects, direct attacks declined in
favor of monster battles, and Toon Defense activated too late. Table's
selection priorities came from inspecting the policy and local card scripts;
card selections are not alternatives explored by this search.

## Development and rejected alternatives

Development used seed `5620000` against the 13 reference decks, initially
128 games per opponent. The retained package then played 512 per opponent
(6,656 games per pilot):

| Pilot | Before | Retained package | Gain [95% CI], points |
|---|---:|---:|---:|
| Watt | 17.95% | 20.67% | +2.72 [+2.06, +3.38] |
| Toon | 19.97% | 23.84% | +3.87 [+3.19, +4.55] |

An expanded package was compared on exactly those same games. It added early
Burden of the Mighty and broader Wattkeeper use for Watt; deferred Toon World,
Gemini Elf summon priority and multi-tribute Cannon Soldier lethal for Toon.
It improved over the retained package by only **+0.17 [-0.24, +0.57]** and
**+0.05 [-0.50, +0.61]** points respectively. Those additions were discarded.
Setting direct attackers when no Battle Phase was available and prioritizing
Toon Dark Magician Girl's immediate attack also failed initial screening.

## Search headroom

Before/after logged searches use eight games per reference opponent, seed
`4510000`, default 8/32/96-world stages and strict hidden-information checks:
104 games per pilot per version. This is a small discovery sample, not a hard
ceiling. The held-out comparison above is the evidence for the improvements.

| Pilot | Version | Alone | With search | Gap |
|---|---|---:|---:|---:|
| Watt | before | 15.4% | 30.8% | 15.4 pp |
| Watt | after | 22.1% | 37.5% | 15.4 pp |
| Toon | before | 15.4% | 25.0% | 9.6 pp |
| Toon | after | 21.2% | 30.8% | 9.6 pp |

The gaps did not shrink in these samples: the search benefited from the
improved pilot too. There is still substantial headroom. All 705,936 before/
after playouts succeeded, and no search duel reached the decision limit.

## Ranking after this round

[DECK-TIER-LIST.md](../DECK-TIER-LIST.md) was regenerated from 119,040 games
(256 per pair, seed `730000`), with zero failures and decision-limit draws.
Toon is **27th at 23.6%**, and Watt **29th at 20.5%** against the complete
pool. The canonical tier list now includes the [Watt follow-up](watt-followup-rules.md);
this section and its linked summary retain the first round's measurements. These rates use different seeds from the held-out comparison and
include both improved opponents.

All **103,936 games involving neither Watt nor Toon** reproduced the previous
round's response transcripts and outcomes exactly. The Ojama and Destiny HERO
policies are unchanged. Rebuilding the ranking from saved results reproduced
the Markdown byte-for-byte.

- [Paired, search and screening results](watt-toon-2026-10-06.summary.json)
- [Source/binary metadata and raw-result hashes](watt-toon-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-watt-toon.summary.json)
- [Ranking metadata](round-robin-2026-10-06-watt-toon.metadata.json)

## Reproduction

The frozen baseline and raw results remain local in
`/tmp/ygo-watt-toon.peaamnfm`; large JSONL and replay logs are not added to Git.
To reconstruct the baseline in a separate checkout, retain the prior
Ojama/Destiny HERO changes and use `watt.rs` and `toon.rs` from `bc8c0f7`.
Build each library with `cargo build --release -p ygo-policies-ffi` and the
runner with `cargo build --release -p ygo-policies-bench`. With `$BASELINE`,
`$CANDIDATE` and a fresh `$OUT` directory:

```sh
target/release/policy-bench compare \
  --policies watt,toon --opponents all --games 512 --seed 6730000 --workers 8 \
  --baseline "$BASELINE" --candidate "$CANDIDATE" \
  --output "$OUT/heldout.jsonl"

# Run separately for each library, using a different output filename.
target/release/policy-bench search \
  --policies watt,toon --opponents existing --games 8 --seed 4510000 \
  --worlds 8 --confirm 32 --final 96 --strict true --log true --record true \
  --library "$CANDIDATE" --workers 16 --output "$OUT/search.jsonl"

target/release/policy-bench round-robin \
  --policies all --games 256 --seed 730000 --workers 12 \
  --library "$CANDIDATE" --output "$OUT/ranking.jsonl" \
  --markdown DECK-TIER-LIST.md
```

Validation: **77 workspace tests passed**, including four new regression tests
for Wattcube's conversion threshold, direct-attack target memory, Toon Table's
choices and Toon Defense's chain/yes-no behavior. The final release build
also reproduced all 960 comparison transcripts against the frozen candidate
on separate seeds after restoring the selected rules and updating comments.
