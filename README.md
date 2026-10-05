# yugioh-policies

Rule-based, single-deck Yu-Gi-Oh! policies that play through OCGCore's own
message protocol (the edo9300 `ygopro-core` used by EDOPro).

A policy is one duel seat. You feed it the engine's messages, and when a message
asks it to decide, it returns the response bytes for OCGCore. It rebuilds the
board itself from those events and never needs anything else from the host:

```text
OCGCore / EDOPro message ─► redact (what this seat may see) ─► projection (board from events)
                                                              └► selection? ─► policy ─► response bytes
```

The seat is fed the raw, unfiltered stream and does its own **redaction**. It
applies the same per-player filtering as an EDOPro server: the other player's
draws, Set cards, hand and prompts are hidden before the projection sees them.
A policy cannot cheat, even when a host hands it everything. Streams that
EDOPro already filtered pass through unchanged.

## Policies

| id | deck (`decks/`) | plan |
| --- | --- | --- |
| `blackwing` | Blackwing Assassin | Blackwing swarm, Synchro follow-ups |
| `burn` | Burn Princess | effect damage and stall |
| `rock-block` | 06 Koaki Meiru - Rock Block | Skill Drain control, trap wall |
| `monarch` | Emperor, Arise! | Tribute Summon Monarchs and card advantage |
| `lightsworn` | Lightsworn Judgment | mill into Judgment Dragon |
| `infernity` | Infernity Infinity | empty-hand combos |
| `gladiator` | Fight, Gladiators! | Gladiator Beast tag-outs |
| `gishki` | Undersea Ceremony | Gishki Ritual Summons (exact-Level Tributes, Forbidden Arts on the opponent's monsters) |
| `crystal` | 12 Crystal Beast - Rainbow | Crystal Beasts stored as Continuous Spells, summoned back; Rainbow Dragon |
| `morphtronic` | 01 Morphtronic - Straight Up | Battle Position as the main lever (each monster has an Attack- and a Defense-Position effect); Power Tool Dragon |
| `dragunity` | Dragunity Flight | small Dragons equipped as Spells, summoned back out for Synchros |
| `spellcaster` | 14 - Spellcaster's Command | Spell Counters: placed where they pay most, spent on destruction, draws, Endymion |
| `heroes` | Fusion Heroes | Fusion toolbox: Polymerization, Miracle Fusion, Fusion Gate, Super Polymerization, Future Fusion |
| `ojama` | Ojama Brigade | Ojamas into Ojama Delta Hurricane!! and Ojama King; Armed Dragon LV3 to LV7; XYZ-Dragon Cannon |
| `watt` | Watt Grid | small Thunders that attack directly (Wattgiraffe, Wattkey) and punish being destroyed |
| `pyramid` | Pyramid of Light | Sphinxes through Pyramid of Light; Guardian Sphinx's Flip Summon bounce, Des Lacooda's draw |
| `arcana` | Arcana Force Fortune | observed coin results, selective retries, Reversal of Fate / Arcana Call, safe Chariot Sets, Fairy beatdown |
| `toon` | Toon Kingdom | Toon World; Toons attack directly; Tribute Toons from the hand, Scapegoat's sheep as fodder |
| `gravekeeper` | Gravekeeper's Tomb | Necrovalley; Spy and Guard Flip Summons, Descendant's removal |
| `karakuri` | Karakuri Workshop | Machines that must attack; position changes into draws and removal; Shogun Synchros |
| `harpie` | Harpie Sisters | Harpies' Hunting Ground: every "Harpie Lady" Summon destroys a Spell/Trap |
| `fortune-lady` | Fortune Ladies | Spellcasters that gain a Level each turn and float into each other |
| `destiny-hero` | Destiny HEROes | Destiny Draw engine into Destiny HERO - Dogma |
| `six-samurai` | Legendary Six Samurai | free Special Summons, Gateway of the Six, Shi En's Spell/Trap negation |
| `tele-dad` | Tele-DAD | Emergency Teleport Synchros, Dark Armed Dragon |
| `quickdraw-plant` | Quickdraw Plants | Graveyard Plant Tuners into Synchros |
| `machina` | Machina Gadgets | Gadget searches, Machina Fortress, trap wall |
| `x-saber` | X-Sabers | XX-Saber Faultroll swarm into X-Saber Synchros |
| `draconic-might` | Draconic Might | Red-Eyes Darkness Metal Dragon every turn, guarded by Prime Material Dragon, Jinzo and Horus LV8; Armed Dragon and Horus LV lines |
| `countdown` | Claudi-oh's Countdown | Final Countdown behind attack locks kept by Skill Drain; one cover a turn, Cyber Valley and Shining Angel for the rest |
| `verdict` | Claudi-oh's Verdict | Skill Drain beatdown: Beast King Barbaros and big Level 4 monsters behind the era's Traps |

Each policy is written for its own deck list in `decks/`, and every list is
legal under the World Championship 2011 Forbidden & Limited list.  Fifteen of them
(`ojama` to `x-saber`) were added as opponents for *Sands of the Duel* (easy,
mid and hard), and `draconic-might` pilots a player's own deck; its Red-Eyes
Darkness Metal Dragon needs the card's script from ProjectIgnis' `pre-errata/`
folder, which EDOPro loads.  `countdown` and `verdict` are the boss's decks for
*Beat Claudi-oh*, built to rank first: one wins without attacking, the other
by attacking ([how they were built and measured](benchmarks/claudi-oh.md)).
[DECK-TIER-LIST.md](DECK-TIER-LIST.md) ranks every deck by measured strength.

## Layout

- `crates/ygo-policies`: engine-agnostic model (`Observation`, `Decision`), the
  `Policy` trait, the shared decision loop, and one strategy per deck. Shared
  card knowledge (`knowledge.rs`) tells every deck what printed stats do not:
  what battle does not destroy, what becomes of a monster that attacks it,
  what effects cannot reach, which face-up cards a deck runs on. It covers
  every monster of the World Championship 2011 pool (`data/`), generated from
  what the engine does with each one ([Card knowledge](#card-knowledge)), and
  is looked up only by the code the seat can see, so the opponent's face-down
  cards stay unknown.
- `crates/ygo-policies-ocgcore`: the OCGCore front end. It contains the message
  parser, redaction, the event-driven projection, and the decision/response
  encoding, plus `Seat`, the thing you feed. `SqliteCards` reads `cards.cdb`.
- `crates/ygo-policies-ffi`: the C ABI (`libygo_policies.so`/`.a`); see
  `include/ygo_policies.h`.
- `crates/ygo-policies-edopro`: `edopro-bot`, a native EDOPro network client
  that joins a room as a player.
- `crates/ygo-policies-bench`: `policy-bench`, native-engine matchups, paired
  baseline/candidate comparisons, round robins, generated tier lists, the
  card probes that generate the shared card knowledge, and a search on top
  of a pilot that shows where its answers lose games.
- `data/`: the World Championship 2011 card pool and Forbidden & Limited list
  (an EDOPro lflist), and what the engine shows about each of its monsters.

The policy libraries do not link OCGCore. Card data comes from any `cards.cdb`
(EDOPro ships one). The optional benchmark crate builds the pinned engine and
Lua from `vendor/`; it is excluded from the workspace's default build members.

## Using it

### Rust

```rust
let db = Arc::new(SqliteCards::open("cards.cdb")?);
let mut seat = Seat::new(registry::create("monarch", db.clone()).unwrap(), db, Some(0));
seat.feed(&start_message(0, [8000, 8000], [40, 40], [15, 15]))?; // optional, recommended
// after each OCG_DuelProcess:
if let Some(response) = seat.feed_buffer(&ocg_duel_get_message_buffer)? {
    // OCG_DuelSetResponse(duel, response)
}
```

### C / Python / Godot (GDExtension)

```bash
cargo build --release   # target/release/libygo_policies.so
```

```c
YgoPolicy *p = ygo_policy_create("monarch", "cards.cdb", 0);
if (ygo_policy_feed_buffer(p, buffer, length) == 1) {
    size_t n; const uint8_t *response = ygo_policy_response(p, &n);
    OCG_DuelSetResponse(duel, response, n);
}
```

`ygo_policy_last_answer_json` returns the observation, the enumerated
decision, the chosen index, and every candidate response. That is what
imitation learning needs: match the chosen response against your own action
encoding.

Two optional messages make the projection better. Send them the way EDOPro's
server does (details in the header):

- `MSG_START`: the seat, Life Points, and pile sizes. Without it, Life Points
  start at 8000 and the Deck sizes are unknown.
- `MSG_UPDATE_DATA`: `OCG_DuelQueryLocation` results. Without them, ATK/DEF
  are printed values (equips and field spells are not reflected), and the
  seat learns its Extra Deck only from its own prompts.

### EDOPro

Host a room in EDOPro (LAN mode, default port 7911), then:

```bash
cargo run --release --bin edopro-bot -- --policy blackwing --cards ~/EDOPro/cards.cdb
cargo run --release --bin edopro-bot -- --list
```

Options: `--host`, `--port`, `--password`, `--deck`, `--duels N` (reconnects
between duels), `--go-second`, and `--version` (EDOPro's client version, in
case the server rejects the default). Each duel writes `edopro-logs/<time>-<policy>.log`
(a readable account) and `.trace` (every message in and response out). To
replay a trace through the current policy, reporting where its answers differ:

```bash
cargo run --release --bin edopro-bot -- --replay edopro-logs/<duel>.trace --policy blackwing --cards cards.cdb
```

## Tests

```bash
cargo test
```

- `ygo-policies-ocgcore/tests/traces.rs` replays 26 recorded OCGCore duels,
  two for each of the first 13 policies, from both seats. At every decision, the projection must equal
  OCGCore's own viewer-filtered snapshot: every card, code, position, pile,
  Life Points, turn, and chain. Nothing hidden may appear.
- Redaction unit tests cover draws, moves to hidden places, prompts, and
  queries.
- `ygo-policies-edopro/tests/session.rs` runs a scripted EDOPro server through
  the lobby handshake and a prompt.

Win-rate benchmarks against the WC2011 AI live in the host repository that
embeds this one.

## Card knowledge

A policy meets cards its own deck does not play.  What every deck should
know about them is in `ygo_policies::knowledge`: Arcana Force 0 - The Fool is
not destroyed by battle, Ally of Justice Catastor destroys the non-DARK
monster that attacks it before damage calculation, Dark Resonator survives
one battle a turn, Stardust Dragon negates what would destroy it but not a
bounce, Sangan pays its controller back when it is destroyed.

These facts are not written by hand.  `policy-bench knowledge` stages each
of the pool's 2,444 monsters on a plain board against the pinned engine, has
it attacked, aimed at and attacking, and writes down what happened as
`crates/ygo-policies/src/knowledge/pool.rs`; the same in words, with the
cards that need a look, is [data/wc2011-monster-facts.md](data/wc2011-monster-facts.md).
The few things a staged board cannot show (a coin toss at Summon, an effect
that needs an empty hand) are added by hand in `knowledge::facts`.  See the
[benchmark crate guide](crates/ygo-policies-bench/README.md#card-facts-from-the-engine)
and [what it changed](benchmarks/pool-knowledge.md).

A deck that wants the exception still decides it in its own `Strategy` hooks.

## Native benchmarks and rankings

```bash
git submodule update --init --recursive
cargo build --release -p ygo-policies-ffi -p ygo-policies-bench

# Specific matchups, using this checkout's deck lists and pinned engine/data.
target/release/policy-bench matchup --policies arcana --opponents blackwing,monarch \
  --games 256 --output arcana-matchups.jsonl

# All policies, alternating seats; produces rankings, tiers and a matchup matrix.
target/release/policy-bench round-robin --policies all --games 256 \
  --output tournament.jsonl --markdown DECK-TIER-LIST.md
```

See the [benchmark crate guide](crates/ygo-policies-bench/README.md) for paired
comparisons, custom pools, saved-run ranking and reproducibility details, and
for [replaying a duel](crates/ygo-policies-bench/README.md#replaying-a-recorded-duel)
that *Beat Claudi-oh* recorded: the duel told turn by turn with nothing
hidden, and where the policy as it is built now would play it otherwise.
Solemn Judgment and Solemn Warning [weigh their price](benchmarks/solemn-price.md)
instead of waiting for a floor of Life Points.
[What a search on top of a pilot became](benchmarks/search-rules.md) for
Blackwing and Monarch: the rules it gave them, and the ones that did not hold.
[Arcana's measured improvement](benchmarks/arcana.md) was validated on 6,656
held-out pairs against the existing 13 policies. Run all tests, including the
benchmark crate, with `YGO_CARDS_CDB="$PWD/vendor/BabelCdb/cards.cdb" cargo test --workspace`.
