//! Selection messages as policy decisions, and their OCGCore responses.
//!
//! [`prompt`] enumerates every answer a selection message allows as a
//! [`Choice`], paired with the exact response bytes OCGCore expects, so
//! `responses[i]` answers `decision.choices[i]`.  The choices keep the shape
//! the policies were written against (idle commands in engine order, ...).
//! Card and sum selections are the exception: they are decided one card at
//! a time through [`Sequential`].

use ygo_policies::cards::CardDatabase;
use ygo_policies::model::{
    CardRef, Choice, ChoiceKind, Decision, DecisionKind, Hint, Location, Member, Observation, Position,
};

use crate::announce;
use crate::message::{Message, Offered};
use crate::projection::card_ref;
use crate::wire::{error, location, Loc, Result, Writer};

/// Structured prompts can have many legal subsets; beyond this the tail is
/// dropped (subsets are enumerated smallest first, in engine order).
pub const MAX_CHOICES: usize = 4096;

pub struct Prompt {
    pub decision: Decision,
    pub responses: Vec<Vec<u8>>,
    /// Card and sum selections are decided one card at a time (see
    /// [`Sequential`]); `decision` and `responses` are then empty.
    pub sequential: Option<Sequential>,
}

impl Prompt {
    fn new(kind: DecisionKind) -> Self {
        Prompt {
            decision: Decision {
                kind,
                hint: Hint::None,
                minimum: 0,
                maximum: 0,
                selected: Vec::new(),
                subject: None,
                choices: Vec::new(),
            },
            responses: Vec::new(),
            sequential: None,
        }
    }

    fn push(&mut self, choice: Choice, response: Vec<u8>) {
        self.decision.choices.push(choice);
        self.responses.push(response);
    }
}

/// OCGCore `HINTMSG_*` selection hints.
pub fn hint(code: u64) -> Hint {
    match code {
        0 => Hint::None,
        500 => Hint::Release,
        501 => Hint::Discard,
        502 => Hint::Destroy,
        503 => Hint::Banish,
        504 => Hint::ToGraveyard,
        505 => Hint::ReturnToHand,
        506 => Hint::AddToHand,
        507 => Hint::ToDeck,
        508 => Hint::Summon,
        509 => Hint::SpecialSummon,
        510 => Hint::Set,
        511 => Hint::FusionMaterial,
        512 => Hint::SynchroMaterial,
        513 => Hint::XyzMaterial,
        514 => Hint::FaceUp,
        515 => Hint::FaceDown,
        516 => Hint::Attack,
        517 => Hint::Defense,
        518 => Hint::Equip,
        520 => Hint::Control,
        526 => Hint::Confirm,
        531 => Hint::Tribute,
        549 => Hint::AttackTarget,
        550 => Hint::Effect,
        551 => Hint::Target,
        other => Hint::Other(other),
    }
}

/// A prompt card as the seat may see it: our own codes come from the prompt,
/// anything else from the projection (the prompt's codes of other players'
/// cards are blanked by the redaction layer anyway).
fn member(obs: &Observation, loc: Loc, code: u32, value: i64, required: bool) -> Member {
    let at = card_ref(loc);
    let code = if code != 0 { Some(code) } else { obs.card(at).and_then(|c| c.code) };
    Member { at, code, value, required }
}

fn plain(kind: ChoiceKind) -> Choice {
    Choice { kind, card: None, members: Vec::new(), description: 0, place: None }
}

fn with_card(kind: ChoiceKind, card: Member, description: u64) -> Choice {
    Choice { kind, card: Some(card), members: Vec::new(), description, place: None }
}

fn int(value: i32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn command(command: u32, index: usize) -> Vec<u8> {
    int(((index as u32) << 16 | command) as i32)
}

/// Card selection response: `i32` 0 then `u32` count and `u32` indices.
fn indices(picked: &[usize]) -> Vec<u8> {
    let mut w = Writer::default().i32(0).u32(picked.len() as u32);
    for k in picked {
        w = w.u32(*k as u32);
    }
    w.0
}

/// Sum selection response: `i32` 2 then `u32` count and `u8` indices.
fn byte_indices(picked: &[usize]) -> Vec<u8> {
    let mut w = Writer::default().i32(2).u32(picked.len() as u32);
    for k in picked {
        w = w.u8(*k as u8);
    }
    w.0
}

pub fn subsets(n: usize, minimum: usize, maximum: usize) -> Vec<Vec<usize>> {
    fn extend(start: usize, n: usize, size: usize, current: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if out.len() >= MAX_CHOICES {
            return;
        }
        if current.len() == size {
            out.push(current.clone());
            return;
        }
        for next in start..n {
            current.push(next);
            extend(next + 1, n, size, current, out);
            current.pop();
        }
    }
    let mut out = Vec::new();
    for size in minimum..=maximum.min(n) {
        extend(0, n, size, &mut Vec::new(), &mut out);
    }
    out
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n > 5 {
        return vec![(0..n).collect()];
    }
    fn permute(rest: Vec<usize>, prefix: Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if rest.is_empty() {
            out.push(prefix);
            return;
        }
        for (k, value) in rest.iter().enumerate() {
            let mut next = rest.clone();
            next.remove(k);
            let mut p = prefix.clone();
            p.push(*value);
            permute(next, p, out);
        }
    }
    let mut out = Vec::new();
    permute((0..n).collect(), Vec::new(), &mut out);
    out
}

/// OCGCore's legality test for sum prompts (`field::process(SelectSum)`).
fn valid_sum(exact: bool, target: i64, parameters: &[i64]) -> bool {
    fn packed(parameters: &[i64], remaining: i64) -> bool {
        let Some((first, rest)) = parameters.split_first() else { return remaining == 0 };
        let low = first & 0xffff;
        let high = first >> 16;
        (remaining >= low && packed(rest, remaining - low)) || (high > 0 && remaining >= high && packed(rest, remaining - high))
    }
    if exact {
        return packed(parameters, target);
    }
    if parameters.is_empty() {
        return false;
    }
    let (mut min_sum, mut max_sum, mut smallest) = (0, 0, i64::MAX);
    for p in parameters {
        let (low, high) = (p & 0xffff, p >> 16);
        let lo = if high > 0 && high < low { high } else { low };
        min_sum += lo;
        max_sum += high.max(low);
        smallest = smallest.min(lo);
    }
    max_sum >= target && min_sum - smallest < target
}

/// One step of a sequential selection: pick a candidate, finish, or cancel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Pick(usize),
    Finish,
    Cancel,
}

/// A card or sum selection decided one card at a time.
///
/// OCGCore asks for the whole subset in one response, but enumerating every
/// subset is combinatorial (1-5 of 27 cards is 101,583 choices).  Each step
/// is a [`DecisionKind::SelectCards`] / [`DecisionKind::SelectSum`] decision
/// whose choices are one [`ChoiceKind::Toggle`] per candidate that can still
/// complete a legal answer, [`ChoiceKind::Finish`] once the picked set is a
/// legal answer, and [`ChoiceKind::Cancel`] when allowed;
/// `decision.selected` holds the cards picked so far.  Candidate indices are
/// the engine's, so [`Sequential::response`] answers the original prompt.
#[derive(Clone, Debug)]
pub struct Sequential {
    pub kind: DecisionKind,
    pub hint: Hint,
    pub candidates: Vec<Member>,
    pub required: Vec<Member>,
    pub minimum: usize,
    pub maximum: usize,
    pub cancelable: bool,
    /// `(exact, target)` for sum prompts: `exact` bounds the count and needs
    /// the exact total, otherwise at least the target without spare material.
    pub sum: Option<(bool, i64)>,
}

impl Sequential {
    fn prompt(self) -> Prompt {
        let mut prompt = Prompt::new(self.kind);
        prompt.decision.hint = self.hint;
        prompt.sequential = Some(self);
        prompt
    }

    /// The decision for the next step after `picked`, and what each choice does.
    pub fn step(&self, picked: &[usize]) -> (Decision, Vec<Step>) {
        let picked_members: Vec<Member> = picked.iter().map(|k| self.candidates[*k]).collect();
        let mut decision = Decision {
            kind: self.kind,
            hint: self.hint,
            minimum: self.minimum as u32,
            maximum: self.maximum as u32,
            selected: picked_members.iter().map(|m| m.at).collect(),
            subject: None,
            choices: Vec::new(),
        };
        let mut steps = Vec::new();
        for k in self.picks(picked) {
            let mut choice = with_card(ChoiceKind::Toggle, self.candidates[k], 0);
            choice.members =
                self.required.iter().chain(picked_members.iter()).copied().chain([self.candidates[k]]).collect();
            decision.choices.push(choice);
            steps.push(Step::Pick(k));
        }
        if self.valid(picked) {
            let mut choice = plain(ChoiceKind::Finish);
            choice.members = self.required.iter().chain(picked_members.iter()).copied().collect();
            decision.choices.push(choice);
            steps.push(Step::Finish);
        }
        if self.cancelable {
            decision.choices.push(plain(ChoiceKind::Cancel));
            steps.push(Step::Cancel);
        }
        (decision, steps)
    }

    /// Candidates that can still complete a legal answer after `picked`.
    pub fn picks(&self, picked: &[usize]) -> Vec<usize> {
        (0..self.candidates.len())
            .filter(|k| !picked.contains(k))
            .filter(|k| {
                let mut next = picked.to_vec();
                next.push(*k);
                self.completable(&mut next, 0)
            })
            .collect()
    }

    /// OCGCore's response for the picked candidates (engine order).
    pub fn response(&self, picked: &[usize]) -> Vec<u8> {
        let mut sorted = picked.to_vec();
        sorted.sort_unstable();
        match self.sum {
            Some(_) => byte_indices(&sorted),
            None => indices(&sorted),
        }
    }

    pub fn cancel_response() -> Vec<u8> {
        int(-1)
    }

    fn valid(&self, set: &[usize]) -> bool {
        if set.len() > self.maximum {
            return false;
        }
        match self.sum {
            None => set.len() >= self.minimum,
            Some((exact, target)) => {
                if set.len() < self.minimum {
                    return false;
                }
                let parameters: Vec<i64> =
                    self.required.iter().chain(set.iter().map(|k| &self.candidates[*k])).map(|m| m.value).collect();
                valid_sum(exact, target, &parameters)
            }
        }
    }

    /// Whether some superset of `set` (adding candidates from `start` on) is legal.
    fn completable(&self, set: &mut Vec<usize>, start: usize) -> bool {
        if self.valid(set) {
            return true;
        }
        if self.dead(set) {
            return false;
        }
        if self.sum.is_none() {
            // Plain selections only need enough cards.
            return set.len() + (0..self.candidates.len()).filter(|k| !set.contains(k)).count() >= self.minimum;
        }
        for k in start..self.candidates.len() {
            if set.contains(&k) {
                continue;
            }
            set.push(k);
            let found = self.completable(set, k + 1);
            set.pop();
            if found {
                return true;
            }
        }
        false
    }

    /// No superset can become legal: every bound only grows with more cards.
    fn dead(&self, set: &[usize]) -> bool {
        if set.len() > self.maximum {
            return true;
        }
        let Some((exact, target)) = self.sum else { return false };
        let (mut min_sum, mut smallest) = (0, i64::MAX);
        for m in self.required.iter().chain(set.iter().map(|k| &self.candidates[*k])) {
            let (low, high) = (m.value & 0xffff, m.value >> 16);
            let lo = if high > 0 && high < low { high } else { low };
            min_sum += lo;
            smallest = smallest.min(lo);
        }
        if exact {
            min_sum > target
        } else {
            smallest != i64::MAX && min_sum - smallest >= target
        }
    }
}

/// The decision a selection message asks for, or `None` for other messages.
/// `selection_hint` is the last `HINT_SELECTMSG` the engine sent this seat.
pub fn prompt(message: &Message, obs: &Observation, selection_hint: u64, db: &dyn CardDatabase) -> Result<Option<Prompt>> {
    use Message::*;
    let offered = |c: &Offered| member(obs, c.loc, c.code, 0, false);
    let mut p;
    match message {
        SelectIdle(idle) => {
            p = Prompt::new(DecisionKind::Idle);
            let lists = [
                (&idle.summon, ChoiceKind::NormalSummon, 0),
                (&idle.special_summon, ChoiceKind::SpecialSummon, 1),
                (&idle.reposition, ChoiceKind::ChangePosition, 2),
                (&idle.set_monster, ChoiceKind::SetMonster, 3),
                (&idle.set_spell_trap, ChoiceKind::SetSpellTrap, 4),
            ];
            for (list, kind, cmd) in lists {
                for (index, card) in list.iter().enumerate() {
                    p.push(with_card(kind, offered(card), 0), command(cmd, index));
                }
            }
            for (index, effect) in idle.activate.iter().enumerate() {
                let card = member(obs, effect.loc, effect.code, 0, false);
                p.push(with_card(ChoiceKind::Activate, card, effect.description), command(5, index));
            }
            if idle.battle_phase {
                p.push(plain(ChoiceKind::EnterBattle), command(6, 0));
            }
            if idle.end_phase {
                p.push(plain(ChoiceKind::EndTurn), command(7, 0));
            }
            if idle.shuffle_hand {
                p.push(plain(ChoiceKind::ShuffleHand), command(8, 0));
            }
        }
        SelectBattle(battle) => {
            p = Prompt::new(DecisionKind::Battle);
            for (index, effect) in battle.activate.iter().enumerate() {
                let card = member(obs, effect.loc, effect.code, 0, false);
                p.push(with_card(ChoiceKind::Activate, card, effect.description), command(0, index));
            }
            for (index, card) in battle.attack.iter().enumerate() {
                p.push(with_card(ChoiceKind::Attack, offered(card), 0), command(1, index));
            }
            if battle.main2 {
                p.push(plain(ChoiceKind::EnterMain2), command(2, 0));
            }
            if battle.end_phase {
                p.push(plain(ChoiceKind::EndTurn), command(3, 0));
            }
        }
        SelectChain { forced, triggers, chains, .. } => {
            p = Prompt::new(DecisionKind::Chain { forced: *forced, triggers: *triggers });
            for (index, effect) in chains.iter().enumerate() {
                let card = member(obs, effect.loc, effect.code, 0, false);
                p.push(with_card(ChoiceKind::Activate, card, effect.description), int(index as i32));
            }
            if !forced {
                p.push(plain(ChoiceKind::Pass), int(-1));
            }
        }
        SelectEffectYesNo { code, loc, description, .. } => {
            p = Prompt::new(DecisionKind::YesNo);
            let card = member(obs, *loc, *code, 0, false);
            p.decision.subject = card.code;
            p.push(with_card(ChoiceKind::Yes, card, *description), int(1));
            p.push(with_card(ChoiceKind::No, card, *description), int(0));
        }
        SelectYesNo { description, .. } => {
            p = Prompt::new(DecisionKind::YesNo);
            p.push(Choice { description: *description, ..plain(ChoiceKind::Yes) }, int(1));
            p.push(Choice { description: *description, ..plain(ChoiceKind::No) }, int(0));
        }
        SelectOption { options, .. } => {
            p = Prompt::new(DecisionKind::Option);
            for (index, option) in options.iter().enumerate() {
                p.push(Choice { description: *option, ..plain(ChoiceKind::Option) }, int(index as i32));
            }
        }
        SelectPosition { code, positions, .. } => {
            p = Prompt::new(DecisionKind::Position);
            p.decision.subject = (*code != 0).then_some(*code);
            for bit in [1u8, 2, 4, 8] {
                if positions & bit != 0 {
                    let position = Position { face_up: bit & 0x5 != 0, attack: bit & 0x3 != 0 };
                    p.push(plain(ChoiceKind::Position(position)), int(bit as i32));
                }
            }
        }
        SelectCard(cards) | SelectTribute(cards) => {
            let members = cards.cards.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, false)).collect();
            let mut hint = hint(selection_hint);
            if hint == ygo_policies::model::Hint::None && matches!(message, SelectTribute(_)) {
                hint = ygo_policies::model::Hint::Tribute;
            }
            return Ok(Some(Sequential::prompt(Sequential {
                kind: DecisionKind::SelectCards,
                hint,
                candidates: members,
                required: Vec::new(),
                minimum: cards.min as usize,
                maximum: cards.max as usize,
                cancelable: cards.cancelable,
                sum: None,
            })));
        }
        SelectUnselect(cards) => {
            p = Prompt::new(DecisionKind::SelectToggle);
            for (index, card) in cards.cards.iter().enumerate() {
                p.push(with_card(ChoiceKind::Toggle, offered(card), 0), Writer::default().i32(1).i32(index as i32).0);
            }
            if cards.finishable {
                p.push(plain(ChoiceKind::Finish), int(-1));
            }
            if cards.cancelable {
                p.push(plain(ChoiceKind::Cancel), int(-1));
            }
            p.decision.minimum = cards.min;
            p.decision.maximum = cards.max;
            p.decision.selected = cards.fixed.iter().map(|c| card_ref(c.loc)).collect();
        }
        SelectSum { cards, exact, target } => {
            let required = cards.fixed.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, true)).collect();
            let candidates: Vec<Member> =
                cards.cards.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, false)).collect();
            let (minimum, maximum) = if *exact {
                (cards.min as usize, if cards.max == 0 { candidates.len() } else { cards.max as usize })
            } else {
                (0, candidates.len())
            };
            return Ok(Some(Sequential::prompt(Sequential {
                kind: DecisionKind::SelectSum,
                hint: hint(selection_hint),
                candidates,
                required,
                minimum,
                maximum,
                cancelable: false,
                sum: Some((*exact, *target as i64)),
            })));
        }
        SelectPlace { player, count, blocked, .. } => {
            p = Prompt::new(DecisionKind::Place);
            let mut zones = Vec::new();
            for owner in [*player, 1 - *player] {
                for (loc, bits) in [(location::MZONE, 0u32), (location::SZONE, 8)] {
                    for sequence in 0..7u32 {
                        let mut bit = 1u32 << (sequence + bits);
                        if owner != *player {
                            bit <<= 16;
                        }
                        if blocked & bit == 0 {
                            zones.push(Loc { controller: owner, location: loc, sequence, position: 0 });
                        }
                    }
                }
            }
            let count = (*count).max(1) as usize;
            for subset in subsets(zones.len(), count, count) {
                let mut choice = plain(ChoiceKind::Place);
                choice.place = Some(card_ref(zones[subset[0]]));
                choice.members = subset.iter().map(|k| member(obs, zones[*k], 0, 0, false)).collect();
                let mut w = Writer::default();
                for k in &subset {
                    w = w.u8(zones[*k].controller).u8(zones[*k].location).u8(zones[*k].sequence as u8);
                }
                p.push(choice, w.0);
            }
        }
        SelectCounter { count, cards, .. } => {
            p = Prompt::new(DecisionKind::Counter);
            fn allocate(caps: &[u32], total: u32, prefix: &mut Vec<u32>, out: &mut Vec<Vec<u32>>) {
                if out.len() >= MAX_CHOICES {
                    return;
                }
                let Some((cap, rest)) = caps.split_first() else {
                    if total == 0 {
                        out.push(prefix.clone());
                    }
                    return;
                };
                for n in 0..=(*cap).min(total) {
                    prefix.push(n);
                    allocate(rest, total - n, prefix, out);
                    prefix.pop();
                }
            }
            let caps: Vec<u32> = cards.iter().map(|c| c.value).collect();
            let mut allocations = Vec::new();
            allocate(&caps, *count as u32, &mut Vec::new(), &mut allocations);
            for amounts in allocations {
                let mut choice = plain(ChoiceKind::Counter);
                choice.members = cards.iter().zip(&amounts).map(|(c, n)| member(obs, c.loc, c.code, *n as i64, false)).collect();
                let mut w = Writer::default();
                for n in &amounts {
                    w = w.u16(*n as u16);
                }
                p.push(choice, w.0);
            }
        }
        Sort { cards, .. } => {
            p = Prompt::new(DecisionKind::Sort);
            let members: Vec<Member> = cards.iter().map(offered).collect();
            for order in permutations(members.len()) {
                let mut choice = plain(ChoiceKind::Sort);
                choice.members = order.iter().map(|k| members[*k]).collect();
                p.push(choice, order.iter().map(|k| *k as u8).collect());
            }
        }
        AnnounceRace { count, available, .. } => {
            p = Prompt::new(DecisionKind::Announce);
            let bits: Vec<u64> = (0..64).map(|b| 1u64 << b).filter(|b| available & b != 0).collect();
            for subset in subsets(bits.len(), *count as usize, *count as usize) {
                let value = subset.iter().fold(0, |acc, k| acc | bits[*k]);
                p.push(Choice { description: value, ..plain(ChoiceKind::Announce) }, value.to_le_bytes().to_vec());
            }
        }
        AnnounceAttribute { count, available, .. } => {
            p = Prompt::new(DecisionKind::Announce);
            let bits: Vec<u32> = (0..32).map(|b| 1u32 << b).filter(|b| available & b != 0).collect();
            for subset in subsets(bits.len(), *count as usize, *count as usize) {
                let value = subset.iter().fold(0, |acc, k| acc | bits[*k]);
                p.push(Choice { description: value as u64, ..plain(ChoiceKind::Announce) }, value.to_le_bytes().to_vec());
            }
        }
        AnnounceNumber { values, .. } => {
            p = Prompt::new(DecisionKind::Announce);
            for (index, value) in values.iter().enumerate() {
                p.push(Choice { description: *value, ..plain(ChoiceKind::Announce) }, int(index as i32));
            }
        }
        AnnounceCard { player, filter } => {
            p = Prompt::new(DecisionKind::Announce);
            for card in db.all().filter(|c| announce::matches(c, filter)).take(MAX_CHOICES) {
                let named = Member {
                    at: CardRef { controller: *player, location: Location::Other, sequence: 0 },
                    code: Some(card.code),
                    value: 0,
                    required: false,
                };
                p.push(with_card(ChoiceKind::Announce, named, card.code as u64), card.code.to_le_bytes().to_vec());
            }
            if p.decision.choices.is_empty() {
                return Err(error("card announcement needs a card database that can list its cards"));
            }
        }
        RockPaperScissors { .. } => {
            p = Prompt::new(DecisionKind::Other);
            for hand in 1..=3 {
                p.push(Choice { description: hand as u64, ..plain(ChoiceKind::Other) }, int(hand));
            }
        }
        _ => return Ok(None),
    }
    if matches!(p.decision.kind, DecisionKind::SelectCards | DecisionKind::SelectToggle | DecisionKind::SelectSum) {
        p.decision.hint = hint(selection_hint);
        if p.decision.hint == ygo_policies::model::Hint::None && matches!(message, SelectTribute(_)) {
            p.decision.hint = ygo_policies::model::Hint::Tribute;
        }
    }
    if p.decision.choices.is_empty() {
        return Err(error("selection message offers no legal answer"));
    }
    Ok(Some(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(sequence: u32, value: i64) -> Member {
        Member {
            at: CardRef { controller: 0, location: Location::Graveyard, sequence },
            code: Some(1000 + sequence),
            value,
            required: false,
        }
    }

    fn cards(count: u32, minimum: usize, maximum: usize) -> Sequential {
        Sequential {
            kind: DecisionKind::SelectCards,
            hint: Hint::None,
            candidates: (0..count).map(|k| candidate(k, 0)).collect(),
            required: Vec::new(),
            minimum,
            maximum,
            cancelable: false,
            sum: None,
        }
    }

    fn sum(values: &[i64], target: i64, exact: bool) -> Sequential {
        Sequential {
            kind: DecisionKind::SelectSum,
            hint: Hint::None,
            candidates: values.iter().enumerate().map(|(k, v)| candidate(k as u32, *v)).collect(),
            required: Vec::new(),
            minimum: if exact { 1 } else { 0 },
            maximum: values.len(),
            cancelable: false,
            sum: Some((exact, target)),
        }
    }

    fn kinds(steps: &[Step]) -> (Vec<usize>, bool) {
        let picks = steps.iter().filter_map(|s| if let Step::Pick(k) = s { Some(*k) } else { None }).collect();
        (picks, steps.contains(&Step::Finish))
    }

    #[test]
    fn card_selection_offers_finish_once_the_minimum_is_met() {
        let selection = cards(4, 1, 3);
        assert_eq!(kinds(&selection.step(&[]).1), (vec![0, 1, 2, 3], false));
        let (decision, steps) = selection.step(&[2]);
        assert_eq!(kinds(&steps), (vec![0, 1, 3], true));
        assert_eq!(decision.selected.len(), 1);
        assert!(selection.picks(&[2, 0, 3]).is_empty(), "the maximum ends the selection");
        assert_eq!(selection.response(&[3, 0]), indices(&[0, 3]));
    }

    #[test]
    fn zero_card_selection_can_finish_immediately() {
        assert_eq!(kinds(&cards(3, 0, 2).step(&[]).1), (vec![0, 1, 2], true));
    }

    #[test]
    fn exact_sum_only_offers_completable_picks() {
        // 2, 3, 4, 6 with target 6: 3 cannot reach exactly 6.
        let selection = sum(&[2, 3, 4, 6], 6, true);
        assert_eq!(kinds(&selection.step(&[]).1), (vec![0, 2, 3], false));
        assert_eq!(kinds(&selection.step(&[0]).1), (vec![2], false));
        assert!(selection.picks(&[3]).is_empty());
        assert_eq!(kinds(&selection.step(&[3]).1), (vec![], true));
        assert_eq!(selection.response(&[2, 0]), byte_indices(&[0, 2]));
    }

    #[test]
    fn at_least_sum_rejects_redundant_material() {
        // At least 6 from 4, 4, 7: {4, 7} has a redundant 4, so it is never offered.
        let selection = sum(&[4, 4, 7], 6, false);
        assert_eq!(kinds(&selection.step(&[0]).1), (vec![1], false));
        assert_eq!(kinds(&selection.step(&[0, 1]).1), (vec![], true));
        assert_eq!(kinds(&selection.step(&[2]).1), (vec![], true));
    }
}
