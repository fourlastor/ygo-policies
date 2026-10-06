# Rock Block and Tele-DAD

The two weakest remaining eligible policies were Rock Block (#17, 55.8%) and
Tele-DAD (#16, 56.0%). Baseline: `8401a8e0064672dc64feef08beea51c08a46ae38`, after the legal Harpie update.
This batch changes only these policies and their tests, on
`codex/rock-block-tele-dad-policies`. Decks and vendor pointers are unchanged.

## Held-out validation

After selecting the rules, each policy played 512 games against every other
deck on fresh seed `36920000`: 15,360 paired games per policy. Both versions
face the same baseline opponents, starting shuffles, seats, and policy randomness.
Draws count as half a win. These results were not used for further tuning.

| Pilot | Games per version | Before | After | Gain, pp [paired 95% CI] |
|---|---:|---:|---:|---:|
| Rock Block | 15,360 | 55.48% | 61.54% | +6.05 [+5.46, +6.65] |
| Tele-DAD | 15,360 | 55.42% | 57.77% | +2.36 [+1.96, +2.75] |

Rock Block improved against 29 opponents, tied 0, and declined against 1. Tele-DAD improved against 29 opponents, tied 0, and declined against 1. Individual matchup samples are smaller than the aggregate; see the summary for their results.

The sole negative matchup estimates were Rock Block against Verdict
(-1.17 pp, 95% CI [-3.40, +1.06]) and Tele-DAD against Destiny HEROes
(-0.59 pp, [-2.65, +1.48]). Neither was used for further tuning.

The confidence intervals are paired normal approximations. No games failed
or reached the decision limit.

## Rules retained

Rock Block:

- Summon Barbaros without tributes even beside existing monsters, explicitly
  choosing that procedure. Do not attempt it on a full monster field.
- Keep another Rock in hand before Normal Summoning Guardian or Sandman.
  Skill Drain does not waive their upkeep: the pinned scripts mark the
  maintenance effect `EFFECT_FLAG_CANNOT_DISABLE`. Only Barbaros counts as
  one of our monsters whose drawback justifies activating Skill Drain.
- Distinguish Royal Oppression's free face-up activation from its 800-LP
  negation. Get it face-up during the opponent's turn, and negate an opposing
  special summon or summoning effect whenever more than 800 LP remain,
  including during our own turn. Do not negate our own summons.
- Summon Grand Mole against large attackers when Skill Drain is absent.
- Use A/D Changer from the Graveyard before battle when an opposing attacker
  stops our available attackers but its DEF is low enough to beat.

Tele-DAD:

- Send Malicious from the Deck only while its partner is not already visible
  in our hand, field, Graveyard, or banished cards. Otherwise prefer
  Plaguespreader, avoiding two Malicious stranded without a Deck partner.

All decisions use permitted observations, public printed card data, and legal
choices. Neither policy receives engine snapshots, opposing hidden identities,
deck order, or future random outcomes. No shared policy behavior changed.

## Development measurements

Each isolated screen used seed `35810000`, 256 games per reference opponent:
3,072 pairs for Rock Block (12 opponents, excluding itself), and 3,328 for
Tele-DAD (all 13). Gains are exploratory percentage-point estimates.

| Screen | Rock Block candidate | Gain | Tele-DAD candidate | Gain |
|---|---|---:|---|---:|
| 1 | Barbaros without tributes | +2.31 pp | Broader Trooper milling | +0.12 pp |
| 2 | Earlier Skill Drain | -0.29 pp | Avoid milling second Malicious | +2.85 pp |
| 3 | Correct upkeep assumptions | +0.59 pp | Broader Malicious timing | -0.60 pp |
| 4 | Broader Solemn Warning | -1.01 pp | Teleport in Main Phase 2 | -0.03 pp |
| 5 | Oppression negation context | +1.63 pp | Broader DAD targets | +0.03 pp |
| 6 | Broader Magic Jammer | +0.46 pp | Any DARK as Grepher discard | -4.00 pp |
| 7 | Offensive A/D Changer | +0.42 pp | Earlier Caius | +0.00 pp |
| 8 | A/D Changer flips Jar | +0.16 pp | Lower Krebons LP threshold | +0.00 pp |
| 9 | Broader Mystic Box trades | +0.03 pp | Use Psychic Commander | +0.21 pp |
| 10 | Lower Skill Drain LP threshold | +0.07 pp | Sangan searches Plaguespreader | -0.57 pp |
| 11 | Barbaros with full-field guard | +2.31 pp | Malicious checks field too | +2.85 pp |
| 12 | Keep a Rock for upkeep | +1.73 pp | Grepher Normal Summon priority | +0.00 pp |
| 13 | Grand Mole against large attackers | +0.62 pp | Return Malicious with Plaguespreader | +0.21 pp |
| 14 | Set Mystic Box before Jar | -0.42 pp | Preserve useful DAD banish costs | +0.03 pp |
| 15 | Boulder prefers Guardian | +0.29 pp | Grepher Special Summon priority | +1.17 pp |
| 16 | Broader Morphing Jar sets | -0.16 pp | Prefer Stardust | +0.63 pp |
| 17 | Earlier Oppression with negation guard | +2.15 pp | Unchanged | +0.00 pp |

The combined screen gave **Rock Block +7.65 pp** and **Tele-DAD +2.70 pp**. Tele-DAD’s Grepher sequencing change
added no benefit over the Malicious fix alone (+2.85 pp), so the final policy
retains only that selection fix. The larger final development run used
1,024 games per reference opponent: 12,288 Rock Block pairs and 13,312 Tele-DAD
pairs. Gains were **Rock Block +8.07 [+7.38, +8.76] pp** and
**Tele-DAD +2.80 [+2.36, +3.25] pp**. All 6,400 selected-screen scores and decision
digests reproduced in that larger run after code cleanup.

Rules with weak or harmful evidence were omitted. In particular, broader
Warning use and Grepher discard costs lost games; simply preferring Stardust
or changing Teleport, Trooper, Psychic Commander, and Malicious timing did not
provide convincing gains. All screens completed without failed games or
limit draws. Held-out validation, rather than these screens, is the independent
check on the selected combination.

## Search headroom

Before and after searches used seed `34700000`, eight games per reference
opponent, 8/32/96-world stages, strict mode enabled and foresight disabled.

| Pilot | Version | Games | Alone | With search | Gap |
|---|---|---:|---:|---:|---:|
| Rock Block | before | 96 | 51.0% | 69.8% | 18.8 pp |
| Rock Block | after | 96 | 58.3% | 72.9% | 14.6 pp |
| Tele-DAD | before | 104 | 49.0% | 72.1% | 23.1 pp |
| Tele-DAD | after | 104 | 58.7% | 76.0% | 17.3 pp |

Both versions face the same baseline opponents. Each follow-up search uses a
separate library with only its tested policy improved, so Tele-DAD still faces
baseline Rock Block. Metadata records library hashes and source composition.
An ordinary paired comparison reproduced all 200 old/new standalone search
scores and decision digests. Searches had no failed playouts or limit draws.

Search repeatedly chose earlier Oppression activation, alternative summons,
and A/D Changer for Rock Block; Tele-DAD traces highlighted Grepher sequencing,
Teleport timing, and Synchro selection. Selection rules also draw on policy
and pinned-script inspection: search does not explore card-selection choices.
These small samples estimate headroom, not hard ceilings. Search retains
privileged opposing-deck composition and engine random-state access, and its
one-step continuations depend on the pilot, so the estimate is not a monotonic
upper bound as that pilot improves.

## Canonical ranking and checks

The full 31-policy round robin used seed `730000`, 256 games per unordered
pair: **119,040 games**, with no failures or decision-limit draws.

| Pilot | Rank before → after | Win rate before → after |
|---|---|---|
| Rock Block | #17 → #8 | 55.8% → 61.9% |
| Tele-DAD | #16 → #15 | 56.0% → 57.7% |

All **103,936 games involving neither changed policy** reproduced both scores
and decision digests from the preceding Harpie ranking. The generated
[tier list](../DECK-TIER-LIST.md) reproduced byte-for-byte from saved results.
These tournament rates differ from paired held-out rates because seeds and
opponent versions differ.

The workspace passed **147 tests**, including new regressions for
Barbaros procedures and full fields, Koa'ki upkeep under Drain, Oppression
ownership and costs, Grand Mole under Drain, A/D Changer battle targets,
and Malicious milling.

- [Compact evidence](rock-tele-2026-10-06.summary.json)
- [Run metadata and hashes](rock-tele-2026-10-06.metadata.json)
- [Ranking summary](round-robin-2026-10-06-rock-tele.summary.json)
- [Ranking metadata](round-robin-2026-10-06-rock-tele.metadata.json)

Raw JSONL, search traces, and frozen binaries remain outside Git under
`/tmp/ygo-rock-tele.i4x32sg8`; their hashes and exact arguments are recorded.
