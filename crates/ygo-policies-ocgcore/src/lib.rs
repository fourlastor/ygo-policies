//! OCGCore front end for the policies: engine messages in, responses out.
//!
//! A [`Seat`] is one player.  Feed it every message the engine produces, in
//! order, exactly as `OCG_DuelGetMessage` returns them (or as EDOPro's server
//! forwards them); when a message asks this seat to decide, [`Seat::feed`]
//! returns the bytes to pass to `OCG_DuelSetResponse` (or to send back to
//! EDOPro as `CTOS_RESPONSE`).
//!
//! ```text
//!   engine message ──► redact (what this seat may see)
//!                        ──► Projection (board rebuilt from events)
//!                        ──► selection? ──► Decision ──► Policy ──► response bytes
//! ```
//!
//! The seat never needs anything but the message stream.  Two optional
//! messages make the projection richer, and front ends that drive OCGCore
//! directly should send them the way EDOPro's server does:
//!
//! * `MSG_START` ([`message::start_message`]): the seat, starting Life Points
//!   and pile sizes, before the first engine message;
//! * `MSG_UPDATE_DATA`: `[6][player][location][OCG_DuelQueryLocation result]`
//!   (flags [`wire::query::RECOMMENDED`]) for the field after each batch, so
//!   current ATK/DEF (after equips, field spells...) replace printed values.

pub mod announce;
#[cfg(feature = "sqlite")]
pub mod cards;
pub mod decision;
pub mod message;
pub mod projection;
pub mod query;
pub mod redact;
pub mod wire;

use std::sync::Arc;

use ygo_policies::agent::Policy;
use ygo_policies::cards::CardDatabase;
use ygo_policies::model::{Choice, ChoiceKind, Decision, Observation};

#[cfg(feature = "sqlite")]
pub use cards::SqliteCards;
pub use message::{start_message, Message};
pub use projection::Projection;
pub use wire::{ProtocolError, Result};

/// The last decision this seat answered.
#[derive(Clone)]
pub struct Answered {
    pub observation: Observation,
    pub decision: Decision,
    pub responses: Vec<Vec<u8>>,
    pub choice: usize,
    /// For a sequential card/sum selection: the candidates picked, in pick
    /// order (engine indices).  `None` for other decisions.
    pub picks: Option<Vec<usize>>,
    retries: usize,
}

/// One player: a policy fed by the engine's message stream.
pub struct Seat {
    policy: Box<dyn Policy>,
    db: Arc<dyn CardDatabase>,
    projection: Projection,
    selection_hint: u64,
    answered: Option<Answered>,
    /// Our response was the last thing the engine received: a `MSG_RETRY`
    /// now rejects it (and not the other player's).
    awaiting: bool,
}

impl Seat {
    /// `seat` is this player's duel seat (0 goes first), or `None` to learn it
    /// from `MSG_START`.
    pub fn new(policy: Box<dyn Policy>, db: Arc<dyn CardDatabase>, seat: Option<u8>) -> Self {
        Seat { policy, db, projection: Projection::new(seat), selection_hint: 0, answered: None, awaiting: false }
    }

    /// Copy this seat independently, including the pending answer and RNG.
    /// A custom policy may decline to support copying.
    pub fn fork(&self) -> Option<Self> {
        Some(Self {
            policy: self.policy.fork()?,
            db: self.db.clone(),
            projection: self.projection.clone(),
            selection_hint: self.selection_hint,
            answered: self.answered.clone(),
            awaiting: self.awaiting,
        })
    }

    pub fn seat(&self) -> Option<u8> {
        self.projection.seat
    }

    /// Correct the seat mid-duel (a front end that learns it late).
    pub fn set_seat(&mut self, seat: u8) {
        self.projection.seat = Some(seat);
    }

    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    /// What the seat currently sees.
    pub fn observation(&self) -> Option<Observation> {
        self.projection.seat.map(|me| self.projection.observation(me, &*self.db))
    }

    pub fn last_answer(&self) -> Option<&Answered> {
        self.answered.as_ref()
    }

    fn apply_update(&mut self, mut update: Message) -> Result<()> {
        self.awaiting = false;
        redact::filter_update(&mut update, self.projection.seat)?;
        self.projection.apply(&update);
        Ok(())
    }

    /// Decode one field update once, then filter it independently for both seats.
    pub fn feed_update_pair(first: &mut Self, second: &mut Self, bytes: &[u8]) -> Result<()> {
        let update = message::parse(bytes)?;
        first.apply_update(update.clone())?;
        second.apply_update(update)
    }

    /// Feed one engine message (message id byte + body).  Returns the
    /// response when the message asks this seat to decide.
    pub fn feed(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        let awaiting = std::mem::take(&mut self.awaiting);
        if matches!(message.first(), Some(&wire::msg::UPDATE_DATA) | Some(&wire::msg::UPDATE_CARD)) {
            self.apply_update(message::parse(message)?)?;
            return Ok(None);
        }
        let Some(visible) = redact::redact(message, self.projection.seat)? else { return Ok(None) };
        let parsed = message::parse(&visible)?;
        self.projection.apply(&parsed);
        let Some(me) = self.projection.seat else { return Ok(None) };
        match &parsed {
            Message::Hint { kind: wire::hint::SELECTMSG, player, data } if *player == me => {
                self.selection_hint = *data;
                return Ok(None);
            }
            Message::Retry if awaiting => return Ok(self.retry()),
            _ => {}
        }
        if parsed.responder() != Some(me) {
            return Ok(None);
        }
        let observation = self.projection.observation(me, &*self.db);
        let hint = std::mem::take(&mut self.selection_hint);
        let Some(prompt) = decision::prompt(&parsed, &observation, hint, &*self.db)? else { return Ok(None) };
        if let Some(sequential) = prompt.sequential {
            return self.select_sequentially(observation, sequential).map(Some);
        }
        let choice = self.policy.choose(&observation, &prompt.decision).min(prompt.responses.len() - 1);
        let response = prompt.responses[choice].clone();
        self.answered = Some(Answered {
            observation,
            decision: prompt.decision,
            responses: prompt.responses,
            choice,
            picks: None,
            retries: 0,
        });
        self.awaiting = true;
        Ok(Some(response))
    }

    /// Decide a card or sum selection one pick at a time, then answer the
    /// whole prompt at once.
    fn select_sequentially(&mut self, observation: Observation, sequential: decision::Sequential) -> Result<Vec<u8>> {
        let mut picked = Vec::new();
        let cancelled = loop {
            let (step_decision, steps) = sequential.step(&picked);
            if steps.is_empty() {
                return Err(wire::error("selection message offers no legal answer"));
            }
            let choice = self.policy.choose(&observation, &step_decision).min(steps.len() - 1);
            match steps[choice] {
                decision::Step::Cancel => break true,
                decision::Step::Finish => break false,
                decision::Step::Pick(k) => {
                    picked.push(k);
                    if sequential.picks(&picked).is_empty() {
                        break false;
                    }
                }
            }
        };
        // Retry fallbacks: the chosen answer, then cancel when allowed.
        let (final_decision, _) = sequential.step(&picked);
        let mut summary = Decision { choices: Vec::new(), ..final_decision };
        let mut responses = Vec::new();
        let mut answer = Choice { kind: ChoiceKind::Cards, card: None, members: Vec::new(), description: 0, place: None };
        answer.members = sequential.required.iter().chain(picked.iter().map(|k| &sequential.candidates[*k])).copied().collect();
        summary.choices.push(answer);
        responses.push(sequential.response(&picked));
        if sequential.cancelable {
            summary.choices.push(Choice { kind: ChoiceKind::Cancel, card: None, members: Vec::new(), description: 0, place: None });
            responses.push(decision::Sequential::cancel_response());
        }
        let choice = if cancelled { responses.len() - 1 } else { 0 };
        let response = responses[choice].clone();
        self.answered =
            Some(Answered { observation, decision: summary, responses, choice, picks: Some(picked), retries: 0 });
        self.awaiting = true;
        Ok(response)
    }

    /// Feed an `OCG_DuelGetMessage` buffer (`u32` length-prefixed messages).
    /// Returns the response to the selection it ends with, if it is ours.
    pub fn feed_buffer(&mut self, buffer: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut response = None;
        for message in wire::split_messages(buffer)? {
            if let Some(answer) = self.feed(message)? {
                response = Some(answer);
            }
        }
        Ok(response)
    }

    /// The engine rejected our last response: fall back to the most
    /// conservative answers (pass, cancel, finish, no, end turn), then to the
    /// others in order.
    fn retry(&mut self) -> Option<Vec<u8>> {
        let answered = self.answered.as_mut()?;
        answered.retries += 1;
        let safe = |kind: ChoiceKind| {
            matches!(kind, ChoiceKind::Pass | ChoiceKind::Cancel | ChoiceKind::Finish | ChoiceKind::No | ChoiceKind::EndTurn)
        };
        let order: Vec<usize> = (0..answered.responses.len())
            .filter(|i| safe(answered.decision.choices[*i].kind))
            .chain((0..answered.responses.len()).filter(|i| !safe(answered.decision.choices[*i].kind)))
            .filter(|i| *i != answered.choice)
            .collect();
        let index = *order.get((answered.retries - 1) % order.len().max(1)).unwrap_or(&answered.choice);
        self.awaiting = true;
        Some(answered.responses[index].clone())
    }
}
