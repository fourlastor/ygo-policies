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

struct SelectionAlternative {
    response: Vec<u8>,
    choice: Choice,
    picks: Vec<usize>,
    policy: Box<dyn Policy>,
}

impl SelectionAlternative {
    fn fork(&self) -> Option<Self> {
        Some(Self { response: self.response.clone(), choice: self.choice.clone(),
            picks: self.picks.clone(), policy: self.policy.fork()? })
    }
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
    response_overrides: bool,
    /// Snapshot from before the pending choice, made only for search seats.
    before_choice: Option<Box<dyn Policy>>,
    pending_sequential: Option<decision::Sequential>,
    selection_alternatives: Vec<SelectionAlternative>,
}

impl Seat {
    /// Install definition metadata before answering any prompt. Forks retain
    /// the same immutable metadata; replacement during a transaction is rejected.
    pub fn set_effect_profiles(&mut self, profiles: ygo_policies::cards::EffectProfiles) -> Result<()> {
        if self.awaiting || self.answered.is_some() {
            return Err(wire::error("set effect profiles before feeding a selection prompt"));
        }
        if !self.policy.set_effect_profiles(profiles.clone()) {
            return Err(wire::error("policy does not support effect profiles"));
        }
        self.db = Arc::new(ygo_policies::cards::ProfiledCards::new(self.db.clone(), profiles));
        Ok(())
    }

    /// `seat` is this player's duel seat (0 goes first), or `None` to learn it
    /// from `MSG_START`.
    pub fn new(policy: Box<dyn Policy>, db: Arc<dyn CardDatabase>, seat: Option<u8>) -> Self {
        Seat {
            policy, db, projection: Projection::new(seat), selection_hint: 0,
            answered: None, awaiting: false, response_overrides: false, before_choice: None,
            pending_sequential: None, selection_alternatives: Vec::new(),
        }
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
            response_overrides: self.response_overrides,
            before_choice: match &self.before_choice {
                Some(policy) => Some(policy.fork()?),
                None => None,
            },
            pending_sequential: self.pending_sequential.clone(),
            selection_alternatives: self.selection_alternatives.iter()
                .map(SelectionAlternative::fork).collect::<Option<Vec<_>>>()?,
        })
    }

    /// Allow search to replace future answers while keeping policy memory in
    /// sync. Enable before feeding the first prompt; ordinary seats avoid the
    /// extra policy snapshot. A fork inherits this setting and its snapshot.
    pub fn enable_response_overrides(&mut self) -> Result<()> {
        if self.response_overrides {
            return Ok(());
        }
        if self.awaiting {
            return Err(wire::error("enable response overrides before feeding a prompt"));
        }
        if self.policy.fork().is_none() {
            return Err(wire::error("policy does not support copying"));
        }
        self.response_overrides = true;
        Ok(())
    }

    /// Stop saving policy checkpoints for future response overrides. The actual
    /// committed policy, pending answer and RNG are preserved; alternate-choice
    /// checkpoints are discarded. Call only after committing the last searched
    /// response, when future prompts will use the policy's own answers. Search
    /// callers must reject MSG_RETRY before feeding it, since ordinary fallback
    /// behavior does not commit an alternate action through a saved checkpoint.
    pub fn disable_response_overrides(&mut self) {
        self.response_overrides = false;
        self.before_choice = None;
        self.pending_sequential = None;
        self.selection_alternatives.clear();
    }

    /// Replace the pending response with another offered response. Restores
    /// the policy from before its original choice, then commits the action
    /// actually sent to the engine. Errors leave the pending answer unchanged.
    pub fn override_response(&mut self, response: &[u8]) -> Result<()> {
        if !self.awaiting {
            return Err(wire::error("no pending response to override"));
        }
        let answered = self.answered.as_ref().ok_or_else(|| wire::error("no answered decision"))?;
        // Some prompts encode Finish and Cancel identically. Preserve the
        // original policy state when the bytes actually sent do not change.
        if answered.responses[answered.choice] == response {
            return Ok(());
        }
        let index = answered.responses.iter().position(|candidate| candidate == response)
            .ok_or_else(|| wire::error("response is not an offered choice"))?;
        if answered.picks.is_some() {
            let cached = self.selection_alternatives.iter().find(|candidate| candidate.response == response)
                .ok_or_else(|| wire::error("expand sequential alternatives before overriding this response"))?;
            let policy = cached.policy.fork().ok_or_else(|| wire::error("policy does not support copying"))?;
            let picks = cached.picks.clone();
            self.policy = policy;
            let answered = self.answered.as_mut().unwrap();
            answered.choice = index;
            answered.picks = Some(picks);
            return Ok(());
        }
        let mut policy = self.before_choice.as_ref().and_then(|policy| policy.fork())
            .ok_or_else(|| wire::error("response overrides were not enabled before this prompt"))?;
        if !policy.commit_choice(&answered.observation, &answered.decision, index) {
            return Err(wire::error("policy cannot commit an external choice"));
        }
        self.policy = policy;
        self.answered.as_mut().unwrap().choice = index;
        Ok(())
    }

    /// Expand a sequential target/material prompt into a bounded collection
    /// of legal whole responses. Force each initial choice, let the policy
    /// finish its subset, and cache the resulting policy state. Never reads
    /// hidden cards or enumerates the combinatorial power set. At most twelve
    /// candidates are attempted; `last_answer` keeps choices/responses aligned.
    pub fn alternative_responses(&mut self, maximum: usize) -> Result<Vec<Vec<u8>>> {
        if !self.awaiting {
            return Err(wire::error("no pending response to expand"));
        }
        let Some(sequential) = self.pending_sequential.clone() else {
            return Ok(self.answered.as_ref().unwrap().responses.clone());
        };
        let maximum = maximum.clamp(1, 12);
        let observation = self.answered.as_ref().unwrap().observation.clone();
        let original_response = self.answered.as_ref().unwrap().responses[self.answered.as_ref().unwrap().choice].clone();
        let (_, steps) = sequential.step(&[]);
        for first in 0..steps.len().min(maximum) {
            if self.selection_alternatives.len() >= maximum { break; }
            let mut policy = self.before_choice.as_ref().and_then(|policy| policy.fork())
                .ok_or_else(|| wire::error("response overrides were not enabled before this prompt"))?;
            let (picked, cancelled) = Self::run_sequential(&mut *policy, &observation, &sequential, Some(first))?;
            let response = if cancelled { decision::Sequential::cancel_response() } else { sequential.response(&picked) };
            if self.selection_alternatives.iter().any(|candidate| candidate.response == response) { continue; }
            self.selection_alternatives.push(SelectionAlternative {
                response, choice: Self::selection_choice(&sequential, &picked, cancelled), picks: picked, policy,
            });
        }
        let answered = self.answered.as_mut().unwrap();
        answered.responses = self.selection_alternatives.iter().map(|candidate| candidate.response.clone()).collect();
        answered.decision.choices = self.selection_alternatives.iter().map(|candidate| candidate.choice.clone()).collect();
        answered.choice = answered.responses.iter().position(|response| *response == original_response).unwrap();
        Ok(answered.responses.clone())
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
            Message::Retry if awaiting => return self.retry(),
            _ => {}
        }
        if parsed.responder() != Some(me) {
            return Ok(None);
        }
        let observation = self.projection.observation(me, &*self.db);
        let hint = std::mem::take(&mut self.selection_hint);
        let Some(prompt) = decision::prompt(&parsed, &observation, hint, &*self.db)? else { return Ok(None) };
        self.before_choice = if self.response_overrides { self.policy.fork() } else { None };
        self.pending_sequential = None;
        self.selection_alternatives.clear();
        if let Some(sequential) = prompt.sequential {
            self.pending_sequential = self.response_overrides.then(|| sequential.clone());
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
        let (picked, cancelled) = Self::run_sequential(&mut *self.policy, &observation, &sequential, None)?;
        // Retry fallbacks: the chosen answer, then cancel when allowed.
        let (final_decision, _) = sequential.step(&picked);
        let mut summary = Decision { choices: Vec::new(), ..final_decision };
        let mut responses = Vec::new();
        summary.choices.push(Self::selection_choice(&sequential, &picked, false));
        responses.push(sequential.response(&picked));
        if sequential.cancelable {
            summary.choices.push(Self::selection_choice(&sequential, &picked, true));
            responses.push(decision::Sequential::cancel_response());
        }
        let choice = if cancelled { responses.len() - 1 } else { 0 };
        let response = responses[choice].clone();
        if self.response_overrides {
            let policy = self.policy.fork().ok_or_else(|| wire::error("policy does not support copying"))?;
            self.selection_alternatives.push(SelectionAlternative {
                response: response.clone(), choice: summary.choices[choice].clone(), picks: picked.clone(), policy,
            });
        }
        self.answered =
            Some(Answered { observation, decision: summary, responses, choice, picks: Some(picked), retries: 0 });
        self.awaiting = true;
        Ok(response)
    }

    fn selection_choice(sequential: &decision::Sequential, picked: &[usize], cancelled: bool) -> Choice {
        Choice {
            kind: if cancelled { ChoiceKind::Cancel } else { ChoiceKind::Cards }, card: None,
            members: if cancelled { Vec::new() } else {
                sequential.required.iter().chain(picked.iter().map(|k| &sequential.candidates[*k])).copied().collect()
            }, description: 0, place: None,
        }
    }

    fn run_sequential(policy: &mut dyn Policy, observation: &Observation, sequential: &decision::Sequential,
        mut forced_first: Option<usize>) -> Result<(Vec<usize>, bool)> {
        let mut picked = Vec::new();
        let cancelled = loop {
            let (step_decision, steps) = sequential.step(&picked);
            if steps.is_empty() {
                return Err(wire::error("selection message offers no legal answer"));
            }
            let choice = if let Some(index) = forced_first.take() {
                if !policy.commit_choice(observation, &step_decision, index) {
                    return Err(wire::error("policy cannot commit an external selection"));
                }
                index
            } else { policy.choose(observation, &step_decision).min(steps.len() - 1) };
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
        Ok((picked, cancelled))
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
    fn retry(&mut self) -> Result<Option<Vec<u8>>> {
        if self.answered.is_none() { return Ok(None); }
        self.awaiting = true;
        if self.response_overrides && self.pending_sequential.is_some() {
            self.alternative_responses(12)?;
        }
        let answered = self.answered.as_mut().unwrap();
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
        let response = answered.responses[index].clone();
        if self.response_overrides {
            self.override_response(&response)?;
        }
        Ok(Some(response))
    }
}

#[cfg(test)]
mod override_tests {
    use super::*;
    use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};
    use ygo_policies::cards::MemoryCards;
    use wire::{location, msg, position, query, Writer};

    #[derive(Clone)]
    struct RecordingPolicy {
        committed: Vec<usize>,
        observed: Arc<Mutex<Vec<Vec<usize>>>>,
        forks: Arc<AtomicUsize>,
    }

    impl Policy for RecordingPolicy {
        fn choose(&mut self, _obs: &Observation, _decision: &Decision) -> usize {
            self.observed.lock().unwrap().push(self.committed.clone());
            self.committed.push(100);
            0
        }
        fn fork(&self) -> Option<Box<dyn Policy>> {
            self.forks.fetch_add(1, Ordering::Relaxed);
            Some(Box::new(self.clone()))
        }
        fn commit_choice(&mut self, _obs: &Observation, _decision: &Decision, index: usize) -> bool {
            self.committed.push(index);
            true
        }
    }

    fn seat() -> (Seat, Arc<Mutex<Vec<Vec<usize>>>>) {
        let (seat, observed, _) = counted_seat();
        (seat, observed)
    }

    fn counted_seat() -> (Seat, Arc<Mutex<Vec<Vec<usize>>>>, Arc<AtomicUsize>) {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let forks = Arc::new(AtomicUsize::new(0));
        let policy = RecordingPolicy { committed: Vec::new(), observed: observed.clone(), forks: forks.clone() };
        (Seat::new(Box::new(policy), Arc::new(MemoryCards::default()), Some(0)), observed, forks)
    }

    fn prompt() -> Vec<u8> {
        Writer::default().u8(msg::SELECT_OPTION).u8(0).u8(2).u64(10).u64(20).0
    }

    fn select_two() -> Vec<u8> {
        let mut message = Writer::default().u8(msg::SELECT_CARD).u8(0).u8(0).u32(2).u32(2).u32(4);
        for sequence in 0..4 {
            message = message.u32(10 + sequence).loc(wire::Loc {
                controller: 0, location: location::HAND, sequence, position: position::FACEDOWN_DEFENSE,
            });
        }
        message.0
    }

    #[test]
    fn sequential_alternatives_are_legal_aligned_and_restore_cached_policy() {
        let (mut seat, observed) = seat();
        seat.enable_response_overrides().unwrap();
        let baseline = seat.feed(&select_two()).unwrap().unwrap();
        let responses = seat.alternative_responses(6).unwrap();
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[0], baseline);
        let answer = seat.last_answer().unwrap();
        assert_eq!(answer.choice, 0);
        assert_eq!(answer.decision.choices.len(), responses.len());
        for (choice, response) in answer.decision.choices.iter().zip(&responses) {
            let indices: Vec<u32> = response[8..].chunks_exact(4)
                .map(|part| u32::from_le_bytes(part.try_into().unwrap())).collect();
            assert_eq!(u32::from_le_bytes(response[4..8].try_into().unwrap()), 2);
            assert_eq!(indices.len(), 2);
            assert_ne!(indices[0], indices[1]);
            let mut members: Vec<_> = choice.members.iter().map(|member| member.at.sequence).collect();
            members.sort_unstable();
            assert_eq!(members, indices);
        }
        let mut branch = seat.fork().unwrap();
        branch.override_response(&responses[1]).unwrap();
        assert_eq!(branch.last_answer().unwrap().picks, Some(vec![2, 0]));
        assert_eq!(seat.last_answer().unwrap().picks, Some(vec![0, 1]));
        observed.lock().unwrap().clear();
        branch.feed(&prompt()).unwrap();
        seat.feed(&prompt()).unwrap();
        assert_eq!(*observed.lock().unwrap(), vec![vec![2, 100], vec![100, 100]]);
    }

    #[test]
    fn sum_alternatives_obey_engine_totals_and_candidate_bound() {
        let (mut seat, _) = seat();
        seat.enable_response_overrides().unwrap();
        let levels = [2, 3, 4, 5];
        let mut message = Writer::default().u8(msg::SELECT_SUM).u8(0).u8(0)
            .u32(7).u32(2).u32(2).u32(0).u32(levels.len() as u32);
        for (sequence, level) in levels.into_iter().enumerate() {
            message = message.u32(10 + sequence as u32).loc(wire::Loc {
                controller: 0, location: location::MZONE, sequence: sequence as u32, position: position::FACEUP_ATTACK,
            }).u32(level);
        }
        let baseline = seat.feed(&message.0).unwrap().unwrap();
        assert_eq!(seat.alternative_responses(1).unwrap(), vec![baseline]);
        let responses = seat.alternative_responses(12).unwrap();
        assert_eq!(responses.len(), 2);
        for response in responses {
            assert_eq!(u32::from_le_bytes(response[..4].try_into().unwrap()), 2);
            assert_eq!(u32::from_le_bytes(response[4..8].try_into().unwrap()), 2);
            assert_eq!(response[8..].iter().map(|index| levels[*index as usize]).sum::<u32>(), 7);
            seat.override_response(&response).unwrap();
        }
    }

    #[test]
    fn overrides_restore_pre_choice_memory_and_forks_are_independent() {
        let (mut seat, observed) = seat();
        seat.enable_response_overrides().unwrap();
        assert_eq!(seat.feed(&prompt()).unwrap(), Some(0i32.to_le_bytes().to_vec()));
        let mut branch = seat.fork().unwrap();
        branch.override_response(&1i32.to_le_bytes()).unwrap();
        assert_eq!(branch.last_answer().unwrap().choice, 1);
        assert_eq!(seat.last_answer().unwrap().choice, 0);
        branch.feed(&prompt()).unwrap();
        seat.feed(&prompt()).unwrap();
        assert_eq!(*observed.lock().unwrap(), vec![vec![], vec![1], vec![100]]);
    }

    #[test]
    fn plain_tail_preserves_committed_memory_and_answers_without_policy_forks() {
        let (mut reference, observed, forks) = counted_seat();
        reference.enable_response_overrides().unwrap();
        reference.feed(&prompt()).unwrap();
        reference.override_response(&1i32.to_le_bytes()).unwrap();
        let mut plain = reference.fork().unwrap();
        plain.disable_response_overrides();
        plain.disable_response_overrides();
        assert_eq!(plain.last_answer().unwrap().choice, 1);
        assert!(plain.before_choice.is_none());
        assert!(plain.pending_sequential.is_none());
        assert!(plain.selection_alternatives.is_empty());
        assert!(reference.response_overrides);
        assert!(!plain.response_overrides);
        forks.store(0, Ordering::Relaxed);
        for message in [prompt(), select_two(), prompt(), vec![msg::NEW_TURN, 0], prompt()] {
            observed.lock().unwrap().clear();
            let before = forks.load(Ordering::Relaxed);
            let actual = plain.feed(&message).unwrap();
            assert_eq!(forks.load(Ordering::Relaxed), before);
            let actual_memory = observed.lock().unwrap().clone();
            observed.lock().unwrap().clear();
            let expected = reference.feed(&message).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(*observed.lock().unwrap(), actual_memory);
            let extra = usize::from(actual.is_some()) + usize::from(message[0] == msg::SELECT_CARD);
            assert_eq!(forks.load(Ordering::Relaxed) - before, extra);
            assert_eq!(plain.observation(), reference.observation());
            let (actual, expected) = (plain.last_answer().unwrap(), reference.last_answer().unwrap());
            assert_eq!(actual.choice, expected.choice);
            assert_eq!(actual.responses, expected.responses);
            assert_eq!(actual.picks, expected.picks);
            assert_eq!(actual.decision, expected.decision);
        }
        assert_eq!(forks.load(Ordering::Relaxed), 5,
            "four pre-choice and one sequential post-choice fork are absent in the plain tail");
    }

    #[test]
    fn disabling_after_sequential_override_preserves_selected_memory() {
        let (mut reference, observed, forks) = counted_seat();
        reference.enable_response_overrides().unwrap();
        reference.feed(&select_two()).unwrap();
        let responses = reference.alternative_responses(6).unwrap();
        reference.override_response(&responses[1]).unwrap();
        let mut plain = reference.fork().unwrap();
        plain.disable_response_overrides();
        assert_eq!(plain.last_answer().unwrap().picks, Some(vec![2, 0]));
        assert_eq!(plain.last_answer().unwrap().responses, reference.last_answer().unwrap().responses);
        assert!(plain.selection_alternatives.is_empty());
        forks.store(0, Ordering::Relaxed);
        observed.lock().unwrap().clear();
        let actual = plain.feed(&prompt()).unwrap();
        assert_eq!(forks.load(Ordering::Relaxed), 0);
        assert_eq!(actual, reference.feed(&prompt()).unwrap());
        assert_eq!(*observed.lock().unwrap(), vec![vec![2, 100], vec![2, 100]]);
        assert_eq!(forks.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn invalid_or_late_overrides_leave_the_original_policy_unchanged() {
        let (mut seat, observed) = seat();
        seat.feed(&prompt()).unwrap();
        assert!(seat.enable_response_overrides().is_err());
        assert!(seat.override_response(&1i32.to_le_bytes()).is_err());
        assert!(seat.override_response(&99i32.to_le_bytes()).is_err());
        assert_eq!(seat.last_answer().unwrap().choice, 0);
        seat.feed(&prompt()).unwrap();
        assert_eq!(*observed.lock().unwrap(), vec![vec![], vec![100]]);
        seat.feed(&[msg::CHAIN_END]).unwrap();
        assert!(seat.override_response(&0i32.to_le_bytes()).is_err());
    }

    #[test]
    fn retry_commits_the_actual_fallback_on_search_seats() {
        let (mut seat, observed) = seat();
        seat.enable_response_overrides().unwrap();
        seat.feed(&prompt()).unwrap();
        assert_eq!(seat.feed(&[msg::RETRY]).unwrap(), Some(1i32.to_le_bytes().to_vec()));
        assert_eq!(seat.last_answer().unwrap().choice, 1);
        seat.feed(&prompt()).unwrap();
        assert_eq!(*observed.lock().unwrap(), vec![vec![], vec![1]]);
    }

    #[test]
    fn search_observation_keeps_opponent_hand_codes_hidden() {
        let (mut seat, _) = seat();
        seat.enable_response_overrides().unwrap();
        for (player, code) in [(0, 111), (1, 222)] {
            seat.feed(&Writer::default().u8(msg::DRAW).u8(player).u32(1)
                .u32(code).u32(position::FACEDOWN_DEFENSE).0).unwrap();
        }
        // A full engine query is also redacted before becoming observable.
        let record = Writer::default()
            .u16(8).u32(query::CODE).u32(333)
            .u16(8).u32(query::POSITION).u32(position::FACEDOWN_DEFENSE)
            .u16(4).u32(query::END).0;
        let mut update = Writer::default().u8(msg::UPDATE_DATA).u8(1).u8(location::HAND)
            .u32(record.len() as u32).0;
        update.extend(record);
        seat.feed(&update).unwrap();
        let observation = seat.observation().unwrap();
        assert_eq!(observation.pile(0, ygo_policies::model::Location::Hand).next().unwrap().code, Some(111));
        assert_eq!(observation.pile(1, ygo_policies::model::Location::Hand).next().unwrap().code, None);
        assert_eq!(seat.fork().unwrap().observation(), Some(observation));
    }
}
