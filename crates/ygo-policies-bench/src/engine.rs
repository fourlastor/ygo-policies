//! Duels between two policy libraries, and staged duels for the probes.
//! The engine, the Decks and the duel itself are `ygo-policies-duel`'s: what
//! is here seats the players and writes what happened.
use std::{collections::HashMap, path::Path};
use ygo_policies_duel::{Duel, LibrarySeat, Snapshot, State};
pub use ygo_policies_duel::{Asked, Core, Deck, DuelOptions, Monster, Placed, PolicyLibrary, Printed, Recorded, Replayed, Result};
use ygo_policies_ocgcore::{
    message::{self, Message},
    wire::{msg, position, query},
};

mod search;
pub use search::{play_searching, SearchOptions};

#[derive(Clone, Copy)]
pub struct PlayOptions {
    pub seed: u64,
    pub limit: usize,
    pub trace: bool,
    /// Starting Life Points of seat 0 (who goes first) and seat 1.
    pub life_points: [u32; 2],
    /// Also write the duel as a record that `replay` plays again.
    pub record: bool,
}

/// A record in the form `replay` reads, with the policies that played.
fn record_json(recorded: &Recorded, names: [&str; 2]) -> serde_json::Value {
    let mut record = recorded.to_json();
    record["players"] = serde_json::json!(names);
    record
}

/// Life Points of both players in a staged duel.
pub const STAGED_LIFE_POINTS: u32 = 30000;

/// What a staged duel shows its watcher.
pub enum Step<'a> {
    /// Every engine message, in order, with its bytes (id first).
    Message(&'a Message, &'a [u8]),
    /// A selection about to be answered, with both monster zones as they stand.
    Prompt(&'a Message, &'a [Monster]),
}

/// A duel that starts from a staged board instead of two shuffled decks:
/// `cards` are put in place, nobody draws an opening hand, and seat 0
/// takes the first turn with attacks allowed (Master Rule 1 otherwise).
/// Both players start at [`STAGED_LIFE_POINTS`], so that a few attacks
/// in a row do not end the duel.
/// The seats answer in process.  `watch` sees every message, and every
/// selection with the monster zones before it is answered; it returns
/// `false` to end the duel there.  Returns the monster zones at the end.
pub fn stage(
    core: &Core,
    cards: &[Placed],
    seed: u64,
    seats: &mut [ygo_policies_ocgcore::Seat; 2],
    limit: usize,
    watch: &mut dyn FnMut(Step) -> bool,
) -> Result<Vec<Monster>> {
    let options = DuelOptions {
        seed: [seed, 2, 3, 4],
        flags: DuelOptions::MASTER_RULE_1 | DuelOptions::ATTACK_FIRST_TURN,
        life_points: [STAGED_LIFE_POINTS; 2],
        hand: 0,
        per_turn: 1,
        shuffle: None,
    };
    let failed = |e: ygo_policies_ocgcore::ProtocolError| e.to_string();
    let mut duel = core.duel(&options)?;
    for card in cards {
        duel.add(*card);
    }
    for (p, seat) in seats.iter_mut().enumerate() {
        seat.feed(&duel.start_message(p as u8)).map_err(failed)?;
    }
    duel.start();
    let mut decisions = 0;
    loop {
        let step = duel.step()?;
        let mut response = None;
        let mut running = true;
        // The field refresh that comes before a selection, until the watcher
        // has seen the selection itself.
        let mut refresh: Vec<&[u8]> = Vec::new();
        for sent in &step.messages {
            if !running {
                break;
            }
            if sent.refresh {
                refresh.push(&sent.bytes);
                continue;
            }
            let raw = &sent.bytes[..];
            let parsed = message::parse(raw).map_err(failed)?;
            if matches!(parsed, Message::Retry) {
                return Err("Engine rejected response (MSG_RETRY)".into());
            }
            running = watch(Step::Message(&parsed, raw)) && !matches!(parsed, Message::Win { .. });
            if running && message::is_selection(raw[0]) {
                running = watch(Step::Prompt(&parsed, &duel.monsters()));
                if running {
                    for update in &refresh {
                        for seat in seats.iter_mut() {
                            seat.feed(update).map_err(failed)?;
                        }
                    }
                }
            }
            refresh.clear();
            if running {
                for seat in seats.iter_mut() {
                    if let Some(answer) = seat.feed(raw).map_err(failed)? {
                        response = Some(answer);
                    }
                }
            }
        }
        if !running || matches!(step.state, State::Over(_)) || decisions >= limit {
            return Ok(duel.monsters());
        }
        match response {
            Some(answer) => {
                duel.respond(&answer);
                decisions += 1;
            }
            None if step.state == State::Awaiting => return Err("Engine awaiting with no answer".into()),
            None => {}
        }
    }
}

/// A duel between two policies, each from its library: who won and how, the
/// digest of every question and answer, and with `trace` every decision.
pub fn play(
    core: &Core,
    decks: &[Deck; 2],
    policies: [&PolicyLibrary; 2],
    names: [&str; 2],
    cards: &Path,
    run: PlayOptions,
) -> Result<serde_json::Value> {
    let PlayOptions { seed, limit, trace, life_points, record } = run;
    let options = DuelOptions { life_points, ..DuelOptions::seeded(seed) };
    let mut duel = core.deal(&options, decks)?;
    let mut recorded = record.then(|| Recorded::dealt(&options, decks));
    let mut seats = Vec::new();
    for p in 0..2 {
        seats.push(policies[p].seat(names[p], cards, p as i32, seed + p as u64)?);
        seats[p].feed(&duel.start_message(p as u8))?;
    }
    let mut decisions = 0;
    let mut turns = 0u32;
    let mut digest = 0xcbf29ce484222325u64;
    let mut traces = Vec::new();
    let mut activations = HashMap::<u32, u32>::new();
    loop {
        let step = duel.step()?;
        let mut response = None;
        for sent in &step.messages {
            let message = &sent.bytes;
            if sent.refresh {
                for seat in &seats {
                    seat.feed(message)?;
                }
                continue;
            }
            let id = message[0];
            if id == msg::RETRY {
                return Err("Engine rejected response (MSG_RETRY)".into());
            }
            if id == msg::NEW_TURN {
                turns += 1;
            }
            if id == msg::CHAINING && message.len() >= 5 {
                *activations
                    .entry(u32::from_le_bytes(message[1..5].try_into().unwrap()))
                    .or_default() += 1;
            }
            for (p, seat) in seats.iter().enumerate() {
                if let Some(answer) = seat.feed(message)? {
                    if response.is_some() {
                        return Err("Multiple answers in one engine batch".into());
                    }
                    for byte in message.iter().chain(&answer) {
                        digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3);
                    }
                    if trace {
                        traces.push(seat.last_answer()?);
                    }
                    response = Some((p as u8, answer));
                }
            }
        }
        if matches!(step.state, State::Over(_)) || decisions >= limit {
            // 1: Life Points, 2: deck-out, 0x10 and up: a card's own win condition.
            let (winner, reason) = match step.state {
                State::Over(Some(outcome)) => (Some(outcome.winner), Some(outcome.reason)),
                State::Over(None) => return Err("Engine ended without MSG_WIN".into()),
                _ => (None, None),
            };
            let mut row = serde_json::json!({"winner": winner, "reason": reason, "turns": turns, "decisions": decisions,
                "limit": winner.is_none(), "digest": format!("{digest:016x}"), "trace": traces, "activations": activations});
            if let Some(recorded) = &recorded {
                row["record"] = record_json(recorded, names);
            }
            return Ok(row);
        }
        if let Some((player, answer)) = response {
            duel.respond(&answer);
            if let Some(recorded) = &mut recorded {
                recorded.responses.push((player, answer));
            }
            decisions += 1;
        } else if step.state == State::Awaiting {
            return Err("Engine awaiting with no policy answer".into());
        }
    }
}
