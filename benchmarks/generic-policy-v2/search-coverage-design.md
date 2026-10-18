# Unknown facedown-monster search: bounded replay design

**UNIMPLEMENTED — read-only feasibility assessment, 2026-10-14.** No coverage recovery, latency or playing-strength improvement has been demonstrated. This design does not relax the live search guards. The user accepts known opponent deck composition; the opponent's actual hidden arrangement and private choices remain unavailable to search.

## Judgment

A bounded Rust prototype is feasible using existing native snapshots, but a general solution is substantially larger. The useful prototype must condition observed draws explicitly and reconstruct only a short, supported public action sequence. Ordinary rejection sampling with the current small world pools is impractical. Even with draw conditioning, the smallest prefix covers only simple Set-and-pass turns, not most flip-engine transactions. It is not yet a compelling Deckout strength intervention.

Known composition supplies a distribution of possible cards. It does not describe a field card's summon history, registered effects, retained Lua references or pending native selection pointers. Consequently, it does not make swapping an established unknown monster safe.

## Existing boundaries and APIs

Paths below are repository-relative; `engine/` abbreviates `crates/ygo-policies-bench/src/engine/`.

| Source/API | Existing contract and consequence |
| --- | --- |
| `crates/ygo-policies-bench/src/engine/fast.rs`, `play_fast` | Cached MCTS/beam continuations execute before the chain/unknown-monster guards. New roots still require a safe sampler. Per-game replay state would live beside `tree` and `public_memory`. |
| `engine/search.rs`, `HiddenWorlds::with_known` / `restore` | Canonical sampling covers own Deck and opposing Deck/Hand/facedown SZONE; never MZONE. It retains a query snapshot and seed-to-swap plans for one anchor. |
| `engine/fast.rs`, `public_opponent` / `prepare` | The opponent model is reconstructed from spectator history, then receives sampled field updates. The real opponent's private seat is never copied. `prepare` currently forks the **current** viewer seat, so replay needs an anchor-seat variant. |
| `engine/ismcts.rs`, `World`, `search`, `evaluate_frozen_in_place` | Search already consumes prepared native snapshots plus policy seats. A matched replay world can use this interface without new MZONE swapping. Proposal and validation worlds must remain separate. |
| `crates/ygo-policies-duel/src/duel.rs`, `snapshot`, `restore`, `reseed`, `arena_used_bytes`, `Snapshot::byte_len` | Snapshots are available only at an awaiting-response boundary before submission. Future RNG can be reseeded. Native image sizes can be checked before allocating another snapshot. |
| `vendor/ocgcore/ocgapi.cpp`, `OCG_DuelRestoreSnapshot` | A snapshot belongs to one arena and can restore only into its original duel. Images remain independent while live play advances; they cannot be transferred to a different duel or worker. |
| `vendor/ocgcore/duel.cpp`, `make_snapshot` / `restore_snapshot` | The snapshot owns a complete external byte image. Restoring rewinds the arena allocation cursor; all internal pointers retain their original addresses. Drop retained images before the duel/core libraries leave scope. |
| `crates/ygo-policies-duel/src/policy.rs`, `LibrarySeat::fork`, `enable_response_overrides`, `override_response` | Policy memory lives outside the native snapshot. Replay needs independent seats, with overrides enabled before their first replayed prompt. Committing a forced legal response must also commit policy state. |

Native restore alone is not the obstacle: existing snapshot fixtures cover chain responses and card selections. `vendor/ocgcore/duel_swap.cpp` excludes MZONE, and its chain check covers effect handlers rather than every target group/Lua reference. `operations.cpp`'s normal-Set processor establishes summon metadata and executes adjustment/event callbacks. A public-looking untouched Set is not a proof that no internal references exist.

## Per-game state and new Rust interface

Retain one safe **anchor**, rather than keeping every sampled world throughout the game. At an eligible later root, restore that anchor, sample there, replay the observed prefix, and keep matched worlds. This permits independently seeded validation worlds without permanently retaining two large pools.

Illustrative interfaces, not existing implementation:

```rust
struct ReplayAnchor<'a> {
    native: Snapshot,
    viewer: LibrarySeat<'a>,
    public_opponent: LibrarySeat<'a>,
    original_prompt: serde_json::Value,
    known_slots: Vec<(u8, u32, u32)>,
    tape: Vec<VisibleReplayEvent>,
}

fn condition_world(
    duel: &mut Duel,
    anchor: &ReplayAnchor<'_>,
    prefix: &SupportedPrefix,
    seed: u64,
    limits: &ReplayLimits,
) -> Result<Option<World<'_>>>;
```

The lifetime spelling above is schematic; returned seats borrow policy libraries, not the anchor. `None` means an unsupported or nonmatching sampled world. Errors remain errors and must be counted. Before any replay, save the live root and restore it on every success, rejection, deadline and error path. Live `Table` seats/logs must never receive simulated messages. After exact current-prompt matching, bind a fresh fork of the current real **viewer** seat to the accepted world, preserving its actual known history and pending-answer cache; retain the simulated opponent seat, never the real opponent's seat.

Anchor capture must not depend on whether normal search found two candidates: an EndTurn-only prompt can still be a useful anchor. Replace or discard the single anchor after an unsupported event, size limit, expiry or a newer eligible anchor.

## What transcript exists, and what must be added

`Table.logs` stores length-prefixed **raw** engine messages, including private prompts and updates. It does not store submitted responses. `ismcts::Path` stores a digest, not a replayable sequence. Optional `Recorded.responses` contains both players' private responses and must not become this replay input.

Add a typed tape at the existing `play_fast` message/response hooks:

- Redact actual engine events with `redact(raw, Some(viewer))` before interpreting or storing them.
- Store the viewer's own visible prompt and committed response immediately before `duel.respond`.
- Store no actual opponent response, private prompt, hand identity or private policy state. Resolve opponent actions using legal choices generated **inside each sampled world** and the observed public movement/Set/phase events. A source hand slot may constrain replay only when it occurs in a viewer-visible event.
- Preserve `Sent.refresh` separately. `Duel::step` injects field refreshes before *every* selection, including opponent-private selections. Literal refresh-count matching would reject equivalent public sequences merely because sampled private prompt counts differ. Normalize redundant refreshes, preserve actual visible state changes, and require exact current viewer observation/legal-action agreement before accepting a root. Do not reuse `Path` as the prefix matcher without addressing this distinction.

This normalization and sampled-action resolution are new work; the current source does not supply a public replay engine.

## Smallest honest supported prefix

Start from a viewer Idle prompt whose chosen action is EndTurn, with no active chain, unsafe pending selection or unknown opposing MZONE card. EndTurn is administrative, so sampling does not invalidate a pending card-choice pointer. Sample **before** submitting it.

Initially permit only normal phase transitions, ordinary draws as specified below, one opponent normal Set from hand without tributes, public placement, legal passes and opponent EndTurn. Stop at the viewer's next unchained Idle prompt. Reject unsupported activations, summons, battle, deck manipulation, discards, reveals, extra draws, control changes and other transactions. Extra S/T Sets are a possible later extension, not implied support: their sampled legal actions and placement must also be reconstructed and validated.

Forced/private intermediate choices are not copied from the real game. Resolve only explicitly supported administrative choices from public evidence; reject other choices. Reject sampled worlds whose native execution produces a different public sequence or current prompt.

This misses the opponent's opening Set when the searcher goes second and no earlier suitable viewer response exists. An opponent Idle snapshot cannot simply substitute: after hand resampling its cached `msetable_cards`/other choice vectors can refer to the old hand. Initial/pre-start snapshots are also not currently supported by `can_snapshot`. It also does not extend automatically through several turns containing unknown monsters.

Deckout's current specialist uses Upstart/Hand Destruction/Card Destruction, Taiyou, Books and Shallow Grave (`crates/ygo-policies/src/decks/deckout.rs`). Those sequences exceed this prefix. Pure Set/pass coverage is plausible; its frequency is unmeasured.

## Own-draw conditioning is required

For four independently sampled worlds, one observed draw of a one-copy card from 30 cards survives in at least one world with probability

`1 - (29/30)^4 = 12.68%`.

Two independent proposal/validation pools each retaining at least one world survive with probability approximately `1.61%`, even before Set legality and transcript mismatches. Requiring several worlds is worse. These are illustrative calculations, not measured acceptance rates.

The useful prefix therefore needs a new conditional sampler: use the **now observed** own draw from the viewer tape, sample the remaining own Deck uniformly subject to that draw, and place that card at the native draw end before replay. Native Draw consumes `list_main.back()` (`operations.cpp`). For one draw with no intervening own-deck manipulation, a uniformly shuffled unfrozen deck followed by a uniform choice among matching copies to swap into the required draw slot gives the conditional distribution. Respect already-known/frozen slots; inconsistent constraints reject the prefix. Never preserve the actual unknown remaining deck order.

This is hindsight conditioning on information available at the current root, not access to an unobserved future draw. It is only sound for the supported no-manipulation prefix. Multiple draws, searches, shuffles, returns or deck-top effects need additional semantics. Until conditioning is implemented and tested, support **only prefixes before any draw**; ordinary turn-spanning coverage must not be claimed.

Opponent Set legality still causes rejection. Resolve it in the sampled engine, rather than choosing the real set identity or reusing the real private action. Conditioning on legal public actions is a model of possible worlds, not an inferred probability model of the specialist's hidden decisions.

## Bounded work and memory proposal

Use one anchor and one bounded tape (for example 256 KiB), at most four retained proposal worlds, and stream independently sampled validation worlds after releasing proposal images. Count anchor, query-cache, current-live, branch and world images against one budget. A conservative prototype could cap logical native images at 32 MiB/game and refuse unavailable size telemetry. `arena_used_bytes` allows checking before capture; `byte_len` verifies accounting afterward. This is an image budget, **not a complete RSS limit**: policy forks, Rust metadata and the live arena also consume memory.

Completed v17 search telemetry provides scale only: per-root maximum snapshot-image sizes had median 3.35 MiB / p95 8.01 MiB across the roster; Deckout's 195 searched roots had median 2.08 MiB / maximum 2.84 MiB. These maxima include search branches, are not retained-anchor measurements, and cannot predict replay memory. Six 3 MiB images would already consume 18 MiB before policy forks.

Cap replay attempts and responses separately (illustrative maxima: 64 world attempts and 32 responses per attempt), bound tape age to the supported single-turn interval, and share the decision deadline with subsequent search. Record all replay work; it is not free preprocessing. Under a small search budget, reserve adequate validation work first and fall back rather than consume the entire budget producing a proposal. Unsupported/depleted pools cause no live-action change. No 200 ms claim is justified without measurement.

## Required validation before any coverage claim

1. **Hidden-layout invariance:** paired native fixtures with identical viewer histories/compositions but different real hand/deck/set identities yield the same conditioned worlds, eligibility, decisions and deterministic counters for fixed seeds. No real opponent response bytes enter the tape.
2. **Normal-Set semantics:** sampled native legality/procedure/placement creates the monster; incompatible candidates reject without mutating live play. No direct MZONE swap occurs.
3. **Draw conditioning:** one-copy and duplicate-copy cases, frozen slots, impossible constraints and remaining-deck permutation invariance. No peek at unobserved tail order.
4. **Transcript matching:** equivalent redundant refreshes, sampled private prompt differences, public divergence and exact current viewer prompt/response legality.
5. **Restoration and host state:** success, rejection, timeout, engine error and depleted pools preserve live native continuation and both real policy seats/logs.
6. **Budgets and independence:** snapshot drop/accounting, bounded tape/forks/work, missing size APIs and independently seeded proposal/validation pools.
7. **Coverage instrumentation before strength evaluation:** eligible anchors, unsupported prefixes by cause, draw-conditioned attempts, matching worlds, pool depletion, recovered roots, replay time and image bytes. Existing `facing_set` counts do not establish how many skipped roots this prefix can recover.

**Do not implement the shortcuts:** remove the facedown guard; retain the actual set identity while sampling the rest; stop only when a reveal/opponent prompt appears; inspect the secret card to decide whether it is safe; replay actual opponent private responses; or extend MZONE swapping by copying placement fields alone. A narrow replay prototype is technically possible, but meaningful Deckout coverage remains an open empirical question.
