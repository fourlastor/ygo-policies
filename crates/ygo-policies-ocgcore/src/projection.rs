//! The seat's own board state, rebuilt from the engine's event stream.
//!
//! [`Projection::apply`] consumes messages that already went through
//! [`crate::redact`], so it can only ever learn what the seat could see: a
//! code is remembered once it was public (a monster that was face-up and got
//! bounced to the hand is still known), and forgotten when the engine hides
//! it again (a shuffled hand).
//!
//! Cards are addressed like OCGCore addresses them: controller, location,
//! sequence.  Piles (hand, Graveyard, banished, Extra Deck) close ranks when a
//! card leaves, zones keep their slots.

use ygo_policies::cards::CardDatabase;
use ygo_policies::model::{CardRef, CardView, ChainLink, Coin, CoinEffect, CoinToss, Location, Observation, Phase, Position};

use crate::message::Message;
use crate::query::Query;
use crate::wire::{location, position, Loc};

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Card {
    pub code: Option<u32>,
    pub position: u32,
    /// Current stats, when the front end forwards card queries.
    pub attack: Option<i32>,
    pub defense: Option<i32>,
    pub level: Option<u32>,
    pub counters: u32,
    /// Declared an attack this turn.
    pub attacked: bool,
    /// Damage calculations it has been through this turn.
    pub battles: u32,
    pub materials: Vec<Card>,
    pub coin_effect: Option<CoinEffect>,
}

impl Card {
    fn known(code: u32, position: u32) -> Card {
        Card { code: (code != 0).then_some(code), position, ..Card::default() }
    }

    fn face_up(&self) -> bool {
        self.position & position::FACEUP != 0
    }

    fn refresh(&mut self, query: &Query) {
        if let Some(code) = query.code.filter(|c| *c != 0) {
            self.code = Some(code);
        }
        if let Some(position) = query.position {
            self.position = position;
            if !self.face_up() { self.coin_effect = None; }
        }
        if query.attack.is_some() {
            self.attack = query.attack;
        }
        if query.defense.is_some() {
            self.defense = query.defense;
        }
        match (query.level, query.rank) {
            (_, Some(rank)) if rank > 0 => self.level = Some(rank),
            (Some(level), _) => self.level = Some(level),
            _ => {}
        }
        if let Some(counters) = query.counters {
            self.counters = counters;
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Side {
    pub deck: u32,
    pub hand: Vec<Card>,
    pub monsters: [Option<Card>; 7],
    pub spells: [Option<Card>; 8],
    pub grave: Vec<Card>,
    pub banished: Vec<Card>,
    pub extra: Vec<Card>,
}

impl Side {
    fn pile(&mut self, loc: u8) -> Option<&mut Vec<Card>> {
        match loc {
            location::HAND => Some(&mut self.hand),
            location::GRAVE => Some(&mut self.grave),
            location::REMOVED => Some(&mut self.banished),
            location::EXTRA => Some(&mut self.extra),
            _ => None,
        }
    }

    fn zone(&mut self, loc: u8, sequence: u32) -> Option<&mut Option<Card>> {
        match loc {
            location::MZONE => self.monsters.get_mut(sequence as usize),
            location::SZONE => self.spells.get_mut(sequence as usize),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Link {
    pub code: u32,
    pub controller: u8,
    pub source: Loc,
    pub targets: Vec<Loc>,
    /// Still being activated (costs and targets not yet final).
    pub building: bool,
    /// Public identities at targeting time, even after a target is banished.
    pub target_codes: Vec<Option<u32>>,
    pub negated: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Projection {
    /// The seat this projection belongs to, once known.
    pub seat: Option<u8>,
    pub turn: u32,
    pub turn_player: Option<u8>,
    pub phase: u16,
    pub life_points: [i32; 2],
    pub sides: [Side; 2],
    pub chain: Vec<Link>,
    pub attacker: Option<Loc>,
    pub attack_target: Option<Loc>,
    /// The card(s) being Summoned: what a "when a monster is Summoned" response answers.
    pub summoning: Vec<(Loc, u32)>,
    summoning_open: bool,
    /// The turn player has Normal Summoned or Set a monster this turn.
    pub summon_used: bool,
    /// Monsters the pending Battle Phase prompt lets attack.
    pub attackable: Option<Vec<Loc>>,
    pub coin_toss: Option<CoinToss>,
    resolving: Option<usize>,
}

impl Projection {
    pub fn new(seat: Option<u8>) -> Self {
        Projection { seat, life_points: [8000, 8000], ..Projection::default() }
    }

    fn take(&mut self, at: Loc) -> Card {
        let Some(side) = self.sides.get_mut(at.controller as usize) else { return Card::default() };
        match at.location {
            location::DECK => {
                side.deck = side.deck.saturating_sub(1);
                Card::default()
            }
            l if l & location::OVERLAY != 0 => {
                let host = side.monsters.get_mut(at.sequence as usize).and_then(Option::as_mut);
                match host {
                    Some(host) if (at.position as usize) < host.materials.len() => host.materials.remove(at.position as usize),
                    _ => Card::default(),
                }
            }
            l => {
                if let Some(pile) = side.pile(l) {
                    let index = at.sequence as usize;
                    if index < pile.len() {
                        pile.remove(index)
                    } else {
                        pile.pop().unwrap_or_default()
                    }
                } else if let Some(slot) = side.zone(l, at.sequence) {
                    slot.take().unwrap_or_default()
                } else {
                    Card::default()
                }
            }
        }
    }

    fn put(&mut self, at: Loc, card: Card) {
        let Some(side) = self.sides.get_mut(at.controller as usize) else { return };
        match at.location {
            location::DECK => side.deck += 1,
            l if l & location::OVERLAY != 0 => {
                if let Some(Some(host)) = side.monsters.get_mut(at.sequence as usize) {
                    let index = (at.position as usize).min(host.materials.len());
                    host.materials.insert(index, card);
                }
            }
            l => {
                if let Some(pile) = side.pile(l) {
                    let index = (at.sequence as usize).min(pile.len());
                    pile.insert(index, card);
                } else if let Some(slot) = side.zone(l, at.sequence) {
                    *slot = Some(card);
                }
            }
        }
    }

    pub fn card_mut(&mut self, controller: u8, loc: u8, sequence: u32) -> Option<&mut Card> {
        let side = self.sides.get_mut(controller as usize)?;
        if matches!(loc, location::HAND | location::GRAVE | location::REMOVED | location::EXTRA) {
            return side.pile(loc)?.get_mut(sequence as usize);
        }
        side.zone(loc, sequence)?.as_mut()
    }

    fn reveal(&mut self, at: Loc, code: u32) {
        if code == 0 {
            return;
        }
        if let Some(card) = self.card_mut(at.controller, at.location, at.sequence) {
            card.code = Some(code);
        }
    }

    fn refresh_location(&mut self, player: u8, loc: u8, records: &[Option<Query>]) {
        let Some(side) = self.sides.get_mut(player as usize) else { return };
        if let Some(pile) = side.pile(loc) {
            // The query lists the whole pile: trust its length.
            pile.resize_with(records.len(), Card::default);
            for (card, record) in pile.iter_mut().zip(records) {
                if let Some(record) = record {
                    card.refresh(record);
                }
            }
            return;
        }
        let zones: &mut [Option<Card>] = match loc {
            location::MZONE => &mut side.monsters,
            location::SZONE => &mut side.spells,
            _ => return,
        };
        for (slot, record) in zones.iter_mut().zip(records) {
            match record {
                None => *slot = None,
                Some(record) => slot.get_or_insert_with(Card::default).refresh(record),
            }
        }
    }

    fn end_battle(&mut self) {
        self.attacker = None;
        self.attack_target = None;
    }

    pub fn apply(&mut self, message: &Message) {
        use Message::*;
        match message {
            Start { seat, observer, life_points, deck, extra } => {
                self.coin_toss = None;
                self.resolving = None;
                if !observer {
                    self.seat = Some(*seat);
                }
                self.life_points = [life_points[0] as i32, life_points[1] as i32];
                for p in 0..2 {
                    self.sides[p] = Side { deck: deck[p] as u32, ..Side::default() };
                    self.sides[p].extra = vec![Card::default(); extra[p] as usize];
                }
            }
            NewTurn { player } => {
                self.coin_toss = None;
                self.turn += 1;
                self.turn_player = Some(*player);
                self.summon_used = false;
                self.end_battle();
                self.summoning.clear();
                for side in &mut self.sides {
                    for card in side.monsters.iter_mut().flatten() {
                        card.attacked = false;
                        card.battles = 0;
                    }
                }
            }
            NewPhase { phase } => {
                self.coin_toss = None;
                self.phase = *phase;
                self.end_battle();
                self.summoning.clear();
            }
            Draw { player, cards } => {
                if let Some(side) = self.sides.get_mut(*player as usize) {
                    side.deck = side.deck.saturating_sub(cards.len() as u32);
                    side.hand.extend(cards.iter().map(|(code, pos)| Card::known(*code, *pos)));
                }
            }
            Move { code, from, to, .. } => {
                let mut card = if from.is_none() { Card::default() } else { self.take(*from) };
                if *code != 0 {
                    card.code = Some(*code);
                }
                // A control change preserves the registered coin; leaving the
                // monster zone or being turned face-down resets it.
                if from.location != location::MZONE || to.location != location::MZONE || to.position & position::FACEUP == 0 {
                    card.coin_effect = None;
                }
                let leaves_field = from.location & (location::MZONE | location::SZONE) != 0
                    && (to.location != from.location || to.controller != from.controller);
                if leaves_field || to.location & (location::MZONE | location::SZONE) == 0 {
                    card.attack = None;
                    card.defense = None;
                    card.level = None;
                    card.counters = 0;
                    card.attacked = false;
                    card.battles = 0;
                    card.materials.clear();
                }
                card.position = to.position;
                if self.attacker == Some(*from) || self.attack_target == Some(*from) {
                    self.end_battle();
                }
                if !to.is_none() {
                    self.put(*to, card);
                }
            }
            PosChange { code, controller, location, sequence, current, .. } => {
                if let Some(card) = self.card_mut(*controller, *location, *sequence as u32) {
                    card.position = *current as u32;
                    if !card.face_up() { card.coin_effect = None; }
                    if *code != 0 {
                        card.code = Some(*code);
                    }
                }
            }
            Set { loc, .. } => {
                if let Some(card) = self.card_mut(loc.controller, loc.location, loc.sequence) {
                    card.position = loc.position;
                }
                if loc.location == location::MZONE && Some(loc.controller) == self.turn_player {
                    self.summon_used = true;
                }
            }
            Swap { first, second } => {
                let mut a = self.take(first.1);
                let mut b = self.take(second.1);
                // Each card ends up where the other one was.
                if first.0 != 0 {
                    a.code = Some(first.0);
                }
                if second.0 != 0 {
                    b.code = Some(second.0);
                }
                a.position = second.1.position;
                b.position = first.1.position;
                self.put(second.1, a);
                self.put(first.1, b);
            }
            Summoning { code, loc } | FlipSummoning { code, loc } => {
                if matches!(message, Summoning { .. }) && Some(loc.controller) == self.turn_player {
                    self.summon_used = true;
                }
                self.reveal(*loc, *code);
                if let Some(card) = self.card_mut(loc.controller, loc.location, loc.sequence) {
                    card.position = loc.position;
                }
                self.summoning = vec![(*loc, *code)];
                self.summoning_open = false;
            }
            SpecialSummoning { code, loc } => {
                self.reveal(*loc, *code);
                if !self.summoning_open {
                    self.summoning.clear();
                    self.summoning_open = true;
                }
                self.summoning.push((*loc, *code));
            }
            Summoned => self.summoning_open = false,
            Chaining { code, loc, controller, .. } => {
                self.reveal(*loc, *code);
                if let Some(card) = self.card_mut(loc.controller, loc.location, loc.sequence) {
                    if loc.location & (location::MZONE | location::SZONE) != 0 {
                        card.position = loc.position;
                    }
                }
                self.chain.push(Link { code: *code, controller: *controller, source: *loc, targets: Vec::new(), building: true,
                    target_codes: Vec::new(), negated: false });
            }
            BecomeTarget { cards } => {
                let codes: Vec<_> = cards.iter().map(|at| self.card_mut(at.controller, at.location, at.sequence).and_then(|c| c.code)).collect();
                if let Some(link) = self.chain.last_mut().filter(|l| l.building) {
                    link.targets.extend(cards.iter().copied());
                    link.target_codes.extend(codes);
                }
            }
            Chained { .. } => {
                if let Some(link) = self.chain.last_mut() {
                    link.building = false;
                }
            }
            ChainSolving { link } => {
                self.resolving = (*link as usize).checked_sub(1);
                self.coin_toss = None;
            }
            ChainNegated { link } => {
                if let Some(link) = self.chain.get_mut((*link as usize).wrapping_sub(1)) { link.negated = true; }
            }
            ChainSolved { link } => {
                let index = (*link as usize).saturating_sub(1);
                // Reversal changes a Lua flag label without updating its client
                // hint. The stream does not prove whether immunity prevented it:
                // discard the stale hint rather than claim a known flipped coin.
                if let Some(link) = self.chain.get(index).filter(|l| l.code == 36690018 && !l.negated).cloned() {
                    for at in link.targets {
                        if let Some(card) = self.card_mut(at.controller, at.location, at.sequence) { card.coin_effect = None; }
                    }
                }
                self.chain.truncate(index);
                self.resolving = None;
                self.coin_toss = None;
            }
            ChainEnd => {
                self.coin_toss = None;
                self.resolving = None;
                self.chain.clear();
                self.summoning.clear();
            }
            TossCoin { player, results } => {
                self.coin_toss = Some(CoinToss { player: *player, results: results.clone(),
                    source: self.resolving.and_then(|i| self.chain.get(i)).map(|l| ChainLink {
                        code: l.code, controller: l.controller, source: card_ref(l.source),
                        targets: l.targets.iter().copied().map(card_ref).collect(),
                    }) });
            }
            CardHint { loc, kind, description } if matches!(description, 62 | 63) => {
                // Arcana.RegisterCoinResult publishes the effective coin. Arcana
                // Call registers the donor's effect; its end-phase restoration
                // publishes another hint outside that resolving chain link.
                let copied = self.resolving.and_then(|i| self.chain.get(i))
                    .filter(|l| l.code == 99189322 && !l.negated)
                    .and_then(|l| l.target_codes.get(1).copied().flatten());
                if let Some(card) = self.card_mut(loc.controller, loc.location, loc.sequence) {
                    match kind {
                        6 if loc.location == location::MZONE && card.face_up() => {
                            card.coin_effect = copied.or(card.code).map(|code| CoinEffect {
                                code, result: if *description == 62 { Coin::Heads } else { Coin::Tails },
                            });
                        }
                        7 => card.coin_effect = None,
                        _ => {}
                    }
                }
            }
            Damage { player, amount } | PayLpCost { player, amount } => {
                if let Some(lp) = self.life_points.get_mut(*player as usize) {
                    *lp = (*lp - *amount as i32).max(0);
                }
            }
            Recover { player, amount } => {
                if let Some(lp) = self.life_points.get_mut(*player as usize) {
                    *lp += *amount as i32;
                }
            }
            LpUpdate { player, life_points } => {
                if let Some(lp) = self.life_points.get_mut(*player as usize) {
                    *lp = *life_points as i32;
                }
            }
            Counter { controller, location, sequence, count, add, .. } => {
                if let Some(card) = self.card_mut(*controller, *location, *sequence as u32) {
                    card.counters = if *add { card.counters + *count as u32 } else { card.counters.saturating_sub(*count as u32) };
                }
            }
            Attack { attacker, target } => {
                self.attacker = Some(*attacker);
                self.attack_target = *target;
                if let Some(card) = self.card_mut(attacker.controller, attacker.location, attacker.sequence) {
                    card.attacked = true;
                }
            }
            BattleResult { attacker, attack, defense, target, target_attack, target_defense } => {
                if let Some(card) = self.card_mut(attacker.controller, attacker.location, attacker.sequence) {
                    card.attack = Some(*attack as i32);
                    card.defense = Some(*defense as i32);
                    card.battles += 1;
                }
                if let Some(target) = target {
                    if let Some(card) = self.card_mut(target.controller, target.location, target.sequence) {
                        card.attack = Some(*target_attack as i32);
                        card.defense = Some(*target_defense as i32);
                        card.battles += 1;
                    }
                }
            }
            AttackDisabled | DamageStepEnd => self.end_battle(),
            UpdateData { player, location, cards } => self.refresh_location(*player, *location, cards),
            UpdateCard { player, location, sequence, card } => match card {
                Some(query) => {
                    if let Some(existing) = self.card_mut(*player, *location, *sequence as u32) {
                        existing.refresh(query);
                    }
                }
                None => {
                    if let Some(side) = self.sides.get_mut(*player as usize) {
                        if let Some(slot) = side.zone(*location, *sequence as u32) {
                            *slot = None;
                        }
                    }
                }
            },
            ConfirmCards { cards, .. } => {
                for (code, at) in cards {
                    self.reveal(*at, *code);
                }
            }
            ShuffleHand { player, codes } | ShuffleExtra { player, codes } => {
                let hand = matches!(message, ShuffleHand { .. });
                if let Some(side) = self.sides.get_mut(*player as usize) {
                    let pile = if hand { &mut side.hand } else { &mut side.extra };
                    *pile = codes.iter().map(|c| Card::known(*c, 0)).collect();
                }
            }
            ShuffleSetCard { from, to, .. } => {
                let cards: Vec<Card> = from.iter().map(|at| self.take(*at)).collect();
                for (card, at) in cards.into_iter().zip(to) {
                    self.put(*at, card);
                }
            }
            SelectIdle(_) => {
                self.attackable = None;
                self.summoning.clear();
            }
            SelectBattle(battle) => {
                self.attackable = Some(battle.attack.iter().map(|c| c.loc).collect());
                self.summoning.clear();
            }
            _ => {}
        }
    }

    /// What `me` sees, in the policies' vocabulary.
    pub fn observation(&self, me: u8, db: &dyn CardDatabase) -> Observation {
        let mut cards = Vec::new();
        let mut pile_sizes = Vec::new();
        for (p, side) in self.sides.iter().enumerate() {
            let p = p as u8;
            let piles: [(Location, &[Card]); 4] = [
                (Location::Hand, &side.hand),
                (Location::Graveyard, &side.grave),
                (Location::Banished, &side.banished),
                (Location::Extra, &side.extra),
            ];
            for (loc, pile) in piles {
                pile_sizes.push((p, loc, pile.len() as u32));
                if loc == Location::Extra && p != me {
                    continue;
                }
                for (sequence, card) in pile.iter().enumerate() {
                    cards.push(self.view(me, db, CardRef { controller: p, location: loc, sequence: sequence as u32 }, card));
                }
            }
            pile_sizes.push((p, Location::Deck, side.deck));
            for (loc, zone) in [(Location::MonsterZone, &side.monsters[..]), (Location::SpellTrapZone, &side.spells[..])] {
                for (sequence, card) in zone.iter().enumerate() {
                    if let Some(card) = card {
                        cards.push(self.view(me, db, CardRef { controller: p, location: loc, sequence: sequence as u32 }, card));
                    }
                }
            }
        }
        Observation {
            me,
            turn: self.turn,
            turn_player: self.turn_player,
            phase: phase(self.phase),
            life_points: self.life_points,
            cards,
            pile_sizes,
            chain: self
                .chain
                .iter()
                .map(|l| ChainLink {
                    code: l.code,
                    controller: l.controller,
                    source: card_ref(l.source),
                    targets: l.targets.iter().map(|t| card_ref(*t)).collect(),
                })
                .collect(),
            battle_attacker: self.attacker.map(card_ref),
            battle_target: self.attack_target.map(card_ref),
            event_cards: self.summoning.iter().map(|(at, code)| (card_ref(*at), (*code != 0).then_some(*code))).collect(),
            summon_used: self.summon_used,
            chain_known: true,
            can_attack_known: true,
            coin_toss: self.coin_toss.clone(),
        }
    }

    fn view(&self, me: u8, db: &dyn CardDatabase, at: CardRef, card: &Card) -> CardView {
        let seen = at.controller == me || card.face_up() || !matches!(at.location, Location::MonsterZone | Location::SpellTrapZone);
        let code = card.code.filter(|_| seen || at.controller == me);
        let printed = code.and_then(|c| db.card(c));
        let stat = |current: Option<i32>, base: fn(&ygo_policies::cards::CardData) -> i32| {
            code.and(current).or_else(|| printed.map(base)).unwrap_or(0)
        };
        CardView {
            at,
            code,
            position: Position {
                face_up: card.position & position::FACEUP != 0,
                attack: card.position & (position::FACEUP_ATTACK | position::FACEDOWN_ATTACK) != 0,
            },
            attack: stat(card.attack, |d| d.attack),
            defense: stat(card.defense, |d| d.defense),
            level: code.and(card.level).or_else(|| printed.map(|d| d.level)).unwrap_or(0),
            can_attack: self.can_attack(at, card),
            battles: if at.location == Location::MonsterZone { card.battles } else { 0 },
            counters: card.counters,
            coin_effect: card.coin_effect.filter(|_| at.location == Location::MonsterZone && card.face_up() && code.is_some()),
        }
    }

    fn can_attack(&self, at: CardRef, card: &Card) -> bool {
        if at.location != Location::MonsterZone {
            return false;
        }
        if let Some(attackable) = &self.attackable {
            return attackable.iter().any(|l| card_ref(*l) == at);
        }
        let phase = phase(self.phase);
        self.turn_player == Some(at.controller)
            && self.turn > 1
            && phase.map_or(false, |p| matches!(p, Phase::Main1 | Phase::BattleStart | Phase::BattleStep))
            && card.position == position::FACEUP_ATTACK
            && !card.attacked
    }
}

pub fn location_of(bits: u8) -> Location {
    match bits {
        location::DECK => Location::Deck,
        location::HAND => Location::Hand,
        location::MZONE => Location::MonsterZone,
        location::SZONE => Location::SpellTrapZone,
        location::GRAVE => Location::Graveyard,
        location::REMOVED => Location::Banished,
        location::EXTRA => Location::Extra,
        l if l & location::OVERLAY != 0 => Location::Overlay,
        _ => Location::Other,
    }
}

pub fn card_ref(loc: Loc) -> CardRef {
    CardRef { controller: loc.controller, location: location_of(loc.location), sequence: loc.sequence }
}

pub fn phase(bits: u16) -> Option<Phase> {
    Some(match bits {
        0x01 => Phase::Draw,
        0x02 => Phase::Standby,
        0x04 => Phase::Main1,
        0x08 => Phase::BattleStart,
        0x10 => Phase::BattleStep,
        0x20 => Phase::Damage,
        0x40 => Phase::DamageCalculation,
        0x80 => Phase::Battle,
        0x100 => Phase::Main2,
        0x200 => Phase::End,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The turn player's Normal Summon or Set is used up for the turn; a
    /// Special or Flip Summon leaves it.
    #[test]
    fn the_normal_summon_is_used_once_a_turn() {
        let at = |controller, location, position| Loc { controller, location, sequence: 0, position };
        let monster = |controller| at(controller, location::MZONE, position::FACEUP_ATTACK);
        let mut projection = Projection::new(Some(0));
        projection.apply(&Message::NewTurn { player: 1 });
        projection.apply(&Message::SpecialSummoning { code: 1, loc: monster(1) });
        projection.apply(&Message::FlipSummoning { code: 1, loc: monster(1) });
        assert!(!projection.summon_used);
        projection.apply(&Message::Summoning { code: 1, loc: monster(1) });
        assert!(projection.summon_used);
        projection.apply(&Message::NewTurn { player: 0 });
        assert!(!projection.summon_used);
        // A Set monster uses it as well; a Set Spell or Trap does not.
        projection.apply(&Message::Set { code: 0, loc: at(0, location::SZONE, position::FACEDOWN) });
        assert!(!projection.summon_used);
        projection.apply(&Message::Set { code: 0, loc: at(0, location::MZONE, position::FACEDOWN_DEFENSE) });
        assert!(projection.summon_used);
    }
}
