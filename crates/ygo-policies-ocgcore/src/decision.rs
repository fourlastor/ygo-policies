//! Selection messages as policy decisions, and their OCGCore responses.
//!
//! [`prompt`] enumerates every answer a selection message allows as a
//! [`Choice`], paired with the exact response bytes OCGCore expects, so
//! `responses[i]` answers `decision.choices[i]`.  The choices keep the shape
//! the policies were written against (idle commands in engine order, one
//! choice per legal card subset, ...).

use ygo_policies::cards::CardDatabase;
use ygo_policies::model::{
    CardRef, Choice, ChoiceKind, Decision, DecisionKind, Hint, Location, Member, Observation, Position,
};

use crate::announce;
use crate::message::{Cards, Message, Offered};
use crate::projection::card_ref;
use crate::wire::{error, location, Loc, Result, Writer};

/// Structured prompts can have many legal subsets; beyond this the tail is
/// dropped (subsets are enumerated smallest first, in engine order).
pub const MAX_CHOICES: usize = 4096;

pub struct Prompt {
    pub decision: Decision,
    pub responses: Vec<Vec<u8>>,
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

fn card_subsets(prompt: &mut Prompt, obs: &Observation, cards: &Cards) {
    let members: Vec<Member> = cards.cards.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, false)).collect();
    for subset in subsets(members.len(), cards.min as usize, cards.max as usize) {
        let mut choice = plain(ChoiceKind::Cards);
        choice.members = subset.iter().map(|k| members[*k]).collect();
        if let [only] = choice.members[..] {
            choice.card = Some(only);
        }
        prompt.push(choice, indices(&subset));
    }
    if cards.cancelable {
        prompt.push(plain(ChoiceKind::Cancel), int(-1));
    }
    prompt.decision.minimum = cards.min;
    prompt.decision.maximum = cards.max;
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
        SelectChain { forced, chains, .. } => {
            p = Prompt::new(DecisionKind::Chain { forced: *forced });
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
            p = Prompt::new(DecisionKind::SelectCards);
            card_subsets(&mut p, obs, cards);
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
            p = Prompt::new(DecisionKind::SelectSum);
            let required: Vec<Member> =
                cards.fixed.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, true)).collect();
            let members: Vec<Member> =
                cards.cards.iter().map(|c| member(obs, c.loc, c.code, c.value as i64, false)).collect();
            let (minimum, maximum) = if *exact {
                (cards.min as usize, if cards.max == 0 { members.len() } else { cards.max as usize })
            } else {
                (0, members.len())
            };
            for subset in subsets(members.len(), minimum, maximum) {
                let parameters: Vec<i64> =
                    required.iter().chain(subset.iter().map(|k| &members[*k])).map(|m| m.value).collect();
                if !valid_sum(*exact, *target as i64, &parameters) {
                    continue;
                }
                let mut choice = plain(ChoiceKind::Cards);
                choice.members = required.iter().copied().chain(subset.iter().map(|k| members[*k])).collect();
                p.push(choice, byte_indices(&subset));
            }
            p.decision.minimum = cards.min;
            p.decision.maximum = cards.max;
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
