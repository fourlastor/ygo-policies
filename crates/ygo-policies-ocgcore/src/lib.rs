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
use ygo_policies::model::{ChoiceKind, Decision, Observation};

#[cfg(feature = "sqlite")]
pub use cards::SqliteCards;
pub use message::{start_message, Message};
pub use projection::Projection;
pub use wire::{ProtocolError, Result};

/// The last decision this seat answered.
pub struct Answered {
    pub observation: Observation,
    pub decision: Decision,
    pub responses: Vec<Vec<u8>>,
    pub choice: usize,
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

    /// Feed one engine message (message id byte + body).  Returns the
    /// response when the message asks this seat to decide.
    pub fn feed(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        let awaiting = std::mem::take(&mut self.awaiting);
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
        let choice = self.policy.choose(&observation, &prompt.decision).min(prompt.responses.len() - 1);
        let response = prompt.responses[choice].clone();
        self.answered =
            Some(Answered { observation, decision: prompt.decision, responses: prompt.responses, choice, retries: 0 });
        self.awaiting = true;
        Ok(Some(response))
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
