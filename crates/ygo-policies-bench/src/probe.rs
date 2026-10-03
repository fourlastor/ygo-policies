//! Controlled mini-duels against the engine: what a monster really does when
//! it is attacked, when an effect is aimed at it, and when it attacks.
//!
//! A probe stages one monster (the subject) on a plain board, has the other
//! side do one kind of thing to it, lets the subject's controller use every
//! effect of the subject the engine offers, and records what happened.  The
//! engine and the card scripts are the rules, so what a probe shows is true
//! for the staged situation.  It shows nothing about what it does not stage:
//! effects set up by a proper Summon (a coin toss, counters), effects that
//! need particular cards in the hand, Deck or Graveyard, effects on other
//! monsters of the same side.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use ygo_policies::agent::{Outcome, Policy};
use ygo_policies::cards::{CardData, CardDatabase};
use ygo_policies::ctx::Ctx;
use ygo_policies::model::{CardRef, Choice, ChoiceKind, Decision, DecisionKind, Location, Observation};
use ygo_policies::tactics;
use ygo_policies_ocgcore::message::Message;
use ygo_policies_ocgcore::wire::{msg, Loc};
use ygo_policies_ocgcore::Seat;

use crate::engine::{Core, Monster, Placed, Printed, Result, Step};

// Plain monsters of our own: nothing on the probing side has an effect.
/// The monster that attacks the subject; its stats are set per probe.
pub const ATTACKER: u32 = 9_990_001;
/// 1000/1000: fills hands, Decks, Graveyards and the bench.
pub const FILLER: u32 = 9_990_002;
/// 0/0: what the subject attacks.
pub const WEAKLING: u32 = 9_990_003;

// The effects aimed at the subject.
/// A Spell that destroys without targeting.
pub const SMASHING_GROUND: u32 = 97169186;
/// A Spell that targets and destroys.
pub const SOUL_TAKER: u32 = 81510157;
/// A Spell that targets and does not destroy.
pub const BOOK_OF_MOON: u32 = 14087893;
/// Two Spells that have nothing to do with the subject: one gains Life
/// Points, the other inflicts damage.
pub const DIAN_KETO: u32 = 84257639;
pub const OOKAZI: u32 = 19523799;
/// A Trap that targets and destroys.
pub const RAIGEKI_BREAK: u32 = 4178474;
/// A Trap that targets and does not destroy.
pub const COMPULSORY_EVACUATION_DEVICE: u32 = 94192409;
/// Two Traps that have nothing to do with the subject: one draws, the
/// other shields from battle.
pub const JAR_OF_GREED: u32 = 83968380;
pub const WABOKU: u32 = 12607053;
/// A monster effect that targets and destroys.
pub const EXILED_FORCE: u32 = 74131780;
/// A monster effect that targets and does not destroy.
pub const BRIONAC: u32 = 50321796;
/// A monster effect that has nothing to do with the subject.
pub const CANNON_SOLDIER: u32 = 11384280;

pub mod attribute {
    pub const EARTH: u32 = 0x01;
    pub const WATER: u32 = 0x02;
    pub const FIRE: u32 = 0x04;
    pub const WIND: u32 = 0x08;
    pub const LIGHT: u32 = 0x10;
    pub const DARK: u32 = 0x20;
    pub const ALL: [u32; 6] = [EARTH, WATER, FIRE, WIND, LIGHT, DARK];
}

mod loc {
    pub const DECK: u8 = 0x01;
    pub const HAND: u8 = 0x02;
    pub const MZONE: u8 = 0x04;
    pub const SZONE: u8 = 0x08;
    pub const GRAVE: u8 = 0x10;
    pub const REMOVED: u8 = 0x20;
    pub const EXTRA: u8 = 0x40;
}

pub mod pos {
    pub const FACEUP_ATTACK: u32 = 0x1;
    pub const FACEUP_DEFENSE: u32 = 0x4;
    pub const FACEDOWN_DEFENSE: u32 = 0x8;
    pub const DEFENSE: u32 = 0xc;
    pub const FACEDOWN: u32 = 0xa;
}

mod reason {
    pub const DESTROY: u32 = 0x1;
    pub const RELEASE: u32 = 0x2;
    pub const BATTLE: u32 = 0x20;
    pub const COST: u32 = 0x80;
    pub const RULE: u32 = 0x400;
}

const TOKEN: u32 = 0x4000;
/// Fusion, Synchro, Xyz, Link and Token: not Main Deck cards.
const NOT_MAIN_DECK: u32 = 0x4000 | 0x40 | 0x2000 | 0x800000 | 0x4000000;
const SUBJECT_ZONE: u32 = 2;
/// OCGCore's "Attack directly?" prompt.
const ATTACK_DIRECTLY: u64 = 31;

/// Monster Types the probes use (the card database's bits).
pub mod race {
    pub const WARRIOR: u64 = 0x1;
    pub const SPELLCASTER: u64 = 0x2;
    pub const MACHINE: u64 = 0x20;
    pub const DRAGON: u64 = 0x2000;
    /// The Type the fewest effects care about: what the plain attacker is.
    pub const SEA_SERPENT: u64 = 0x40000;
    /// The Types effects single out most often.
    pub const OTHERS: [u64; 4] = [WARRIOR, SPELLCASTER, MACHINE, DRAGON];
}

/// The monster that attacks the subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Attacker {
    pub attack: i32,
    pub level: u32,
    pub attribute: u32,
    pub race: u64,
}

impl Attacker {
    /// Stronger than every printed stat in the pool, and as plain as a
    /// monster gets: EARTH, Level 4, Sea Serpent.
    pub const BASE: Attacker = Attacker { attack: 5100, level: 4, attribute: attribute::EARTH, race: race::SEA_SERPENT };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scenario {
    /// `count` such monsters attack the subject one after the other.
    Attacked { defense: bool, attacker: Attacker, count: u8 },
    /// The other side activates this card `times` times, at the subject
    /// when the card targets.
    Effect { card: u32, times: u8 },
    /// The subject attacks a monster in this position, of this Attribute,
    /// with `stats` ATK and DEF.
    Strike { defense: bool, attribute: u32, stats: i32 },
}

/// Where a card went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Fate {
    #[default]
    Stays,
    /// Destroyed by battle.
    Battle,
    /// Destroyed by an effect.
    Destroyed,
    Banished,
    Hand,
    Deck,
    Tributed,
    /// The other player controls it now.
    Taken,
    /// Left the field some other way.
    Other,
}

fn fate(from: Loc, to: Loc, why: u32) -> Fate {
    if why & reason::BATTLE != 0 {
        Fate::Battle
    } else if to.location == loc::MZONE && to.controller != from.controller {
        Fate::Taken
    } else if to.location == loc::REMOVED {
        Fate::Banished
    } else if to.location == loc::HAND {
        Fate::Hand
    } else if to.location == loc::DECK || to.location == loc::EXTRA {
        Fate::Deck
    } else if why & reason::RELEASE != 0 {
        Fate::Tributed
    } else if why & reason::DESTROY != 0 {
        Fate::Destroyed
    } else {
        Fate::Other
    }
}

/// Damage calculation, as the engine reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calc {
    pub attacker_atk: i32,
    pub target_atk: i32,
    pub target_def: i32,
    pub target_defending: bool,
}

impl Calc {
    /// The stat the attacker's ATK was compared with.
    pub fn stat(&self) -> i32 {
        if self.target_defending { self.target_def } else { self.target_atk }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cards {
    /// Monsters (to the field for gains, from it for losses).
    pub monsters: u8,
    /// Tokens among those monsters.
    pub tokens: u8,
    pub spell_traps: u8,
    pub hand: u8,
}

impl Cards {
    pub fn any(&self) -> bool {
        self.monsters + self.spell_traps + self.hand > 0
    }
}

/// One attack or one activation, and everything up to the next one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Act {
    /// The attack was declared at the subject.
    pub at_subject: bool,
    /// The attack was a direct one: the subject was not a legal target.
    pub direct: bool,
    pub negated: bool,
    pub calc: Option<Calc>,
    /// The attacker's ATK on the board when the attack was declared.
    pub attacker_visible: Option<i32>,
    /// The ATK and DEF on the board, then, of the monster attacked.
    pub target_visible: Option<(i32, i32)>,
    /// The subject became the target of the activated card.
    pub targeted: bool,
    /// The activated card or its effect was negated.
    pub countered: bool,
    pub subject: Fate,
    /// The other card of the interaction: the attacker, the monster the
    /// subject attacked, or the activated card.
    pub counterpart: Fate,
    /// The counterpart left before damage calculation.
    pub before_calc: bool,
    /// The subject's position after it changed.
    pub switched: Option<u32>,
    /// Damage each player took (battle and effect), by seat.
    pub damage: [i32; 2],
    /// Life Points each player paid, by seat.
    pub paid: [i32; 2],
    /// Cards each player got on the field or in hand from elsewhere, by seat.
    pub gained: [Cards; 2],
    /// Cards each player lost from the field or hand, by seat, not counting
    /// the subject, the counterpart and costs.
    pub lost: [Cards; 2],
    /// Cards each player paid as costs, by seat.
    pub spent: [Cards; 2],
    /// Activations of the subject's effects.
    pub effects: u32,
}

/// What one probe showed.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    /// The subject right before the first action; `None` when it had left.
    pub before: Option<Monster>,
    /// What happened before the first action.
    pub prelude: Act,
    pub acts: Vec<Act>,
    pub notes: Vec<&'static str>,
    /// What the shared tactics expected of each attack, where asked.
    pub predicted: Vec<Outcome>,
    /// The subject at the end, if it is still on the field.
    pub after: Option<Monster>,
}

impl Seen {
    pub fn noted(&self, note: &str) -> bool {
        self.notes.iter().any(|n| *n == note)
    }
}

#[derive(Default)]
struct Shared {
    notes: Vec<&'static str>,
    predicted: Vec<Outcome>,
}

/// Printed data for the seats: the real cards plus our plain monsters,
/// with the attacker as this probe defines it.
struct ProbeCards {
    cards: Arc<HashMap<u32, CardData>>,
    attacker: CardData,
    weakling: CardData,
}

impl CardDatabase for ProbeCards {
    fn card(&self, code: u32) -> Option<&CardData> {
        match code {
            ATTACKER => Some(&self.attacker),
            WEAKLING => Some(&self.weakling),
            _ => self.cards.get(&code),
        }
    }

    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        Box::new(self.cards.values())
    }
}

/// The acting seat: does its one thing to its target, declines the rest.
struct Actor {
    scenario: Scenario,
    /// The card the action is aimed at.
    target: CardRef,
    entered_battle: bool,
    attacked: HashSet<u32>,
    activations: u8,
    /// The card whose effects this side uses when offered (`Strike`).
    uses: Option<u32>,
    used: HashMap<u64, u32>,
    db: Arc<ProbeCards>,
    shared: Arc<Mutex<Shared>>,
}

impl Actor {
    fn note(&self, text: &'static str) {
        let mut shared = self.shared.lock().unwrap();
        if !shared.notes.contains(&text) {
            shared.notes.push(text);
        }
    }

    fn predict(&self, obs: &Observation, attacker: CardRef) {
        let ctx = Ctx::new(obs, self.db.as_ref());
        if let (Some(attacker), Some(target)) = (obs.card(attacker), obs.card(self.target)) {
            let outcome = tactics::default_outcome(&ctx, attacker, target, 0);
            self.shared.lock().unwrap().predicted.push(outcome);
        }
    }
}

fn find(d: &Decision, kind: ChoiceKind) -> Option<usize> {
    d.choices.iter().position(|c| c.kind == kind)
}

impl Policy for Actor {
    fn choose(&mut self, obs: &Observation, d: &Decision) -> usize {
        let last = d.choices.len().saturating_sub(1);
        match d.kind {
            DecisionKind::Idle => {
                match self.scenario {
                    Scenario::Attacked { .. } | Scenario::Strike { .. } if !self.entered_battle => {
                        self.entered_battle = true;
                        match find(d, ChoiceKind::EnterBattle) {
                            Some(i) => return i,
                            None => self.note("no-battle-phase"),
                        }
                    }
                    Scenario::Effect { card, times } if self.activations < times => {
                        let wanted = |c: &Choice| c.kind == ChoiceKind::Activate && c.code() == Some(card);
                        match d.choices.iter().position(wanted) {
                            Some(i) => {
                                self.activations += 1;
                                return i;
                            }
                            None => {
                                self.note(if self.activations == 0 { "cannot-activate" } else { "cannot-activate-again" });
                                self.activations = times;
                            }
                        }
                    }
                    _ => {}
                }
                find(d, ChoiceKind::EndTurn).unwrap_or(last)
            }
            DecisionKind::Battle => {
                let attack = d.choices.iter().enumerate().find(|(_, c)| {
                    c.kind == ChoiceKind::Attack
                        && c.at().is_some_and(|at| match self.scenario {
                            Scenario::Strike { .. } => at.sequence == SUBJECT_ZONE && self.attacked.is_empty(),
                            _ => !self.attacked.contains(&at.sequence),
                        })
                });
                match attack {
                    Some((i, choice)) => {
                        let at = choice.at().unwrap();
                        self.attacked.insert(at.sequence);
                        if choice.card.is_some_and(|m| m.value == 1) {
                            self.note("direct-attack-offered");
                        }
                        self.predict(obs, at);
                        return i;
                    }
                    None if self.attacked.is_empty() => self.note("cannot-attack"),
                    None => {}
                }
                find(d, ChoiceKind::EndTurn).or(find(d, ChoiceKind::EnterMain2)).unwrap_or(last)
            }
            DecisionKind::SelectCards | DecisionKind::SelectToggle | DecisionKind::SelectSum => {
                let toggle = |pred: &dyn Fn(&Choice) -> bool| d.choices.iter().position(|c| c.kind == ChoiceKind::Toggle && pred(c));
                if let Some(i) = toggle(&|c| c.at() == Some(self.target)) {
                    return i;
                }
                let enough = d.selected.len() as u32 >= d.minimum.max(1);
                if enough {
                    if let Some(i) = find(d, ChoiceKind::Finish) {
                        return i;
                    }
                }
                // A cost, or a pick from the hand: pay with a plain monster.
                let field = toggle(&|c| c.at().is_some_and(|at| at.location.is_field())).is_some();
                if d.hint.is_cost() || !field {
                    return toggle(&|c| c.code() == Some(FILLER)).or(find(d, ChoiceKind::Toggle)).unwrap_or(0);
                }
                // Asked for a target, and ours is not among them.
                self.note("target-not-offered");
                find(d, ChoiceKind::Cancel).or(find(d, ChoiceKind::Toggle)).unwrap_or(0)
            }
            DecisionKind::Chain { .. } => {
                for (i, c) in d.choices.iter().enumerate() {
                    if c.kind == ChoiceKind::Activate && c.code().is_some() && c.code() == self.uses {
                        let n = self.used.entry(c.description).or_default();
                        if *n < 8 {
                            *n += 1;
                            return i;
                        }
                    }
                }
                find(d, ChoiceKind::Pass).unwrap_or(0)
            }
            DecisionKind::YesNo => {
                // We want the battle, not a direct attack past the monster.
                let direct = d.choices.first().is_some_and(|c| c.description == ATTACK_DIRECTLY);
                let answer = if self.uses.is_some() && !direct { ChoiceKind::Yes } else { ChoiceKind::No };
                find(d, answer).unwrap_or(0)
            }
            _ => 0,
        }
    }
}

/// The subject's controller when it is acted upon: uses every effect of the
/// subject it is offered.
struct Owner {
    me: u8,
    subject: u32,
    used: HashMap<u64, u32>,
}

impl Policy for Owner {
    fn choose(&mut self, _: &Observation, d: &Decision) -> usize {
        let last = d.choices.len().saturating_sub(1);
        match d.kind {
            DecisionKind::Chain { .. } => {
                for (i, c) in d.choices.iter().enumerate() {
                    if c.kind == ChoiceKind::Activate && c.code() == Some(self.subject) {
                        let n = self.used.entry(c.description).or_default();
                        if *n < 8 {
                            *n += 1;
                            return i;
                        }
                    }
                }
                find(d, ChoiceKind::Pass).unwrap_or(0)
            }
            DecisionKind::YesNo => find(d, ChoiceKind::Yes).unwrap_or(0),
            DecisionKind::SelectCards | DecisionKind::SelectToggle | DecisionKind::SelectSum => {
                if d.selected.len() as u32 >= d.minimum.max(1) {
                    if let Some(i) = find(d, ChoiceKind::Finish) {
                        return i;
                    }
                }
                // Aim at the other side unless the pick is something we get or pay.
                if !d.hint.is_gain() && !d.hint.is_cost() {
                    let theirs = |c: &Choice| c.kind == ChoiceKind::Toggle && c.at().is_some_and(|at| at.controller != self.me);
                    if let Some(i) = d.choices.iter().position(theirs) {
                        return i;
                    }
                }
                find(d, ChoiceKind::Toggle).or(find(d, ChoiceKind::Finish)).unwrap_or(0)
            }
            DecisionKind::Idle | DecisionKind::Battle => find(d, ChoiceKind::EndTurn).unwrap_or(last),
            _ => 0,
        }
    }
}

/// The side that only watches: declines everything.
struct Bystander;

impl Policy for Bystander {
    fn choose(&mut self, _: &Observation, d: &Decision) -> usize {
        let last = d.choices.len().saturating_sub(1);
        match d.kind {
            DecisionKind::Chain { .. } => find(d, ChoiceKind::Pass).unwrap_or(0),
            DecisionKind::YesNo => find(d, ChoiceKind::No).unwrap_or(0),
            DecisionKind::Idle | DecisionKind::Battle => find(d, ChoiceKind::EndTurn).unwrap_or(last),
            _ => 0,
        }
    }
}

/// Turns the engine's messages into [`Seen`].
struct Recorder {
    scenario: Scenario,
    subject: u32,
    owner: u8,
    /// Where the subject is: (controller, sequence) in a Monster Zone.
    slot: Option<(u8, u32)>,
    /// Where the current act's counterpart is: (controller, location, sequence).
    counterpart: Option<(u8, u8, u32)>,
    /// Chain links of the acting side's card, by position in the chain.
    links: Vec<u32>,
    tokens: Arc<HashSet<u32>>,
    turns: u32,
    /// The monster zones at the acting seat's last prompt.
    board: Vec<Monster>,
    seen: Seen,
}

impl Recorder {
    fn act(&mut self) -> &mut Act {
        match self.seen.acts.last_mut() {
            Some(act) => act,
            None => &mut self.seen.prelude,
        }
    }

    fn subject_in(&self, monsters: &[Monster]) -> Option<Monster> {
        let (controller, sequence) = self.slot?;
        monsters.iter().copied().find(|m| m.controller == controller && m.sequence == sequence && m.code == self.subject)
    }

    fn moved(&mut self, code: u32, from: Loc, to: Loc, why: u32) {
        let field = |l: u8| l == loc::MZONE || l == loc::SZONE;
        if from.location == loc::MZONE && Some((from.controller, from.sequence)) == self.slot && code == self.subject {
            if to.location == loc::MZONE {
                self.slot = Some((to.controller, to.sequence));
                if to.controller != from.controller {
                    self.act().subject = Fate::Taken;
                }
            } else {
                self.slot = None;
                self.act().subject = fate(from, to, why);
            }
            return;
        }
        if Some((from.controller, from.location, from.sequence)) == self.counterpart {
            if field(to.location) && to.controller == from.controller {
                self.counterpart = Some((to.controller, to.location, to.sequence));
            } else {
                self.counterpart = None;
                // A resolved Spell or Trap goes to the Graveyard by rule, and
                // a monster may be its own effect's cost: neither is a loss.
                if why != reason::RULE && why & reason::COST == 0 {
                    let act = self.act();
                    act.counterpart = fate(from, to, why);
                    act.before_calc = act.calc.is_none();
                }
            }
            return;
        }
        let same_side = from.controller == to.controller;
        let pile = |cards: &mut Cards, location: u8, token: bool| match location {
            loc::MZONE => {
                cards.monsters += 1;
                cards.tokens += token as u8;
            }
            loc::SZONE => cards.spell_traps += 1,
            _ => cards.hand += 1,
        };
        let token = self.tokens.contains(&code);
        let leaves = (field(from.location) || from.location == loc::HAND) && !(same_side && field(to.location));
        if leaves && why & reason::COST != 0 {
            pile(&mut self.act().spent[from.controller as usize], from.location, token);
        } else if leaves && why != reason::RULE {
            pile(&mut self.act().lost[from.controller as usize], from.location, token);
        }
        let arrives = match to.location {
            loc::MZONE => !(same_side && field(from.location)),
            loc::SZONE => !(same_side && (field(from.location) || from.location == loc::HAND)),
            loc::HAND => !(same_side && from.location == loc::HAND),
            _ => false,
        };
        if arrives {
            pile(&mut self.act().gained[to.controller as usize], to.location, token);
        }
    }

    /// `false` ends the probe.
    fn step(&mut self, step: Step) -> bool {
        match step {
            Step::Message(message, raw) => {
                match message {
                    Message::NewTurn { .. } => {
                        self.turns += 1;
                        if self.turns > 1 {
                            return false;
                        }
                    }
                    Message::Win { .. } => {
                        self.seen.notes.push("duel-ended");
                        return false;
                    }
                    Message::Attack { attacker, target } => {
                        let at_subject = target.is_some_and(|t| t.location == loc::MZONE && Some((t.controller, t.sequence)) == self.slot);
                        let (subject_attacks, counterpart) = match self.scenario {
                            Scenario::Strike { .. } => (true, target.map(|t| (t.controller, t.location, t.sequence))),
                            _ => (false, Some((attacker.controller, loc::MZONE, attacker.sequence))),
                        };
                        self.counterpart = counterpart;
                        let on_board = |l: &Loc| self.board.iter().find(|m| m.controller == l.controller && m.sequence == l.sequence);
                        self.seen.acts.push(Act {
                            at_subject: at_subject && !subject_attacks,
                            direct: target.is_none(),
                            attacker_visible: on_board(attacker).map(|m| m.attack),
                            target_visible: target.as_ref().and_then(on_board).map(|m| (m.attack, m.defense)),
                            ..Act::default()
                        });
                    }
                    Message::AttackDisabled => self.act().negated = true,
                    Message::BattleResult { attack, target, target_attack, target_defense, .. } => {
                        self.act().calc = Some(Calc {
                            attacker_atk: *attack as i32,
                            target_atk: *target_attack as i32,
                            target_def: *target_defense as i32,
                            target_defending: target.is_some_and(|t| t.position & pos::DEFENSE != 0),
                        });
                    }
                    Message::Move { code, from, to, reason } => self.moved(*code, *from, *to, *reason),
                    Message::PosChange { controller, location, sequence, current, .. } => {
                        if *location == loc::MZONE && Some((*controller, *sequence as u32)) == self.slot {
                            self.act().switched = Some(*current as u32);
                        }
                    }
                    Message::Chaining { code, loc, controller, size, .. } => {
                        let tool = match self.scenario {
                            Scenario::Effect { card, .. } => *controller != self.owner && *code == card,
                            _ => false,
                        };
                        if tool {
                            self.counterpart = Some((loc.controller, loc.location, loc.sequence));
                            self.links.push(*size);
                            self.seen.acts.push(Act::default());
                        } else if *code == self.subject && *controller == self.owner {
                            self.act().effects += 1;
                        }
                    }
                    Message::ChainNegated { link } if self.links.contains(&(*link as u32)) => self.act().countered = true,
                    Message::Other(id) if *id == msg::CHAIN_DISABLED => {
                        if raw.get(1).is_some_and(|link| self.links.contains(&(*link as u32))) {
                            self.act().countered = true;
                        }
                    }
                    Message::ChainEnd => self.links.clear(),
                    Message::BecomeTarget { cards } => {
                        let slot = self.slot;
                        if cards.iter().any(|c| c.location == loc::MZONE && Some((c.controller, c.sequence)) == slot) {
                            self.act().targeted = true;
                        }
                    }
                    Message::Damage { player, amount } => self.act().damage[*player as usize & 1] += *amount as i32,
                    Message::PayLpCost { player, amount } => self.act().paid[*player as usize & 1] += *amount as i32,
                    Message::Draw { player, cards } => self.act().gained[*player as usize & 1].hand += cards.len() as u8,
                    _ => {}
                }
                true
            }
            Step::Prompt(message, monsters) => {
                let subject = self.subject_in(monsters);
                let done = self.seen.acts.len();
                let acting = match message {
                    Message::SelectIdle(m) => m.player == 0,
                    Message::SelectBattle(m) => m.player == 0,
                    _ => false,
                };
                if !acting {
                    return true;
                }
                self.board = monsters.to_vec();
                if done == 0 && self.seen.before.is_none() {
                    self.seen.before = subject;
                }
                let battle = matches!(message, Message::SelectBattle(_));
                match self.scenario {
                    Scenario::Attacked { count, .. } => {
                        // Back in a Main Phase after the attacks, or nothing left to attack.
                        !((!battle && done > 0) || done >= count as usize || (done > 0 && subject.is_none()))
                    }
                    Scenario::Strike { .. } => done == 0,
                    Scenario::Effect { times, .. } => {
                        let face_up = subject.is_some_and(|m| m.position & pos::FACEDOWN == 0);
                        !(done >= times as usize || (done > 0 && !face_up))
                    }
                }
            }
        }
    }
}

/// Runs probes on one engine.
pub struct Lab {
    core: Core,
    cards: Arc<HashMap<u32, CardData>>,
    tokens: Arc<HashSet<u32>>,
    /// Small Normal Monsters, one per Attribute and per Type: something for
    /// the subject's effects to find in its controller's Deck.
    variety: Vec<u32>,
}

fn plain(attack: i32, defense: i32, level: u32, attribute: u32) -> Printed {
    Printed { kind: 0x11, level, attribute, race: race::SEA_SERPENT, attack, defense, alias: 0 }
}

fn attacker(profile: Attacker) -> Printed {
    Printed { race: profile.race, ..plain(profile.attack, 2000, profile.level, profile.attribute) }
}

fn card_data(code: u32, card: Printed) -> CardData {
    CardData {
        code,
        alias: card.alias,
        kind: card.kind,
        attack: card.attack.max(0),
        defense: card.defense.max(0),
        level: card.level,
        race: card.race as u32,
        attribute: card.attribute,
        setcodes: Vec::new(),
    }
}

impl Lab {
    pub fn new(mut core: Core) -> Result<Self> {
        for tool in TOOLS {
            if core.printed(tool).is_none() {
                return Err(format!("the card database lacks {tool}, which the probes use"));
            }
        }
        core.define(ATTACKER, attacker(Attacker::BASE));
        // The filler stays a Warrior: something for a Warrior search to find.
        core.define(FILLER, Printed { race: race::WARRIOR, ..plain(1000, 1000, 4, attribute::EARTH) });
        core.define(WEAKLING, plain(0, 0, 4, attribute::EARTH));
        let mut cards = HashMap::new();
        let mut tokens = HashSet::new();
        let mut variety = Vec::new();
        let mut seen = HashSet::new();
        for code in core.codes() {
            let card = core.printed(code).unwrap();
            cards.insert(code, card_data(code, card));
            if card.kind & TOKEN != 0 {
                tokens.insert(code);
            }
            let small = card.kind == 0x11 && card.alias == 0 && card.level <= 4 && (0..=1500).contains(&card.attack) && card.defense <= 1500;
            if small && code < ATTACKER && (seen.insert((0, card.attribute as u64)) | seen.insert((1, card.race))) {
                variety.push(code);
            }
        }
        Ok(Lab { core, cards: Arc::new(cards), tokens: Arc::new(tokens), variety })
    }

    /// Stage the subject, run the scenario, and report what happened.
    pub fn run(&mut self, subject: u32, scenario: Scenario) -> Result<Seen> {
        let kind = self.core.printed(subject).ok_or_else(|| format!("unknown card {subject}"))?.kind;
        let striking = matches!(scenario, Scenario::Strike { .. });
        // Seat 0 acts: the other side, or the subject's own when it attacks.
        let (owner, other) = if striking { (0u8, 1u8) } else { (1u8, 0u8) };
        let mut cards: Vec<Placed> = Vec::new();
        let mut put = |controller: u8, code: u32, location: u8, sequence: u32, position: u32| {
            cards.push(Placed { controller, code, location: location as u32, sequence, position });
        };
        let position = match scenario {
            Scenario::Attacked { defense: true, .. } => pos::FACEUP_DEFENSE,
            _ => pos::FACEUP_ATTACK,
        };
        put(owner, subject, loc::MZONE, SUBJECT_ZONE, position);
        // The subject's side: plain monsters at both ends of the Deck (the
        // first turn's draw must not find the subject), two more copies of
        // the subject and a variety of small monsters between them.
        for _ in 0..3 {
            put(owner, FILLER, loc::DECK, 0, pos::FACEDOWN_DEFENSE);
        }
        for _ in 0..if kind & NOT_MAIN_DECK == 0 { 2 } else { 0 } {
            put(owner, subject, loc::DECK, 0, pos::FACEDOWN_DEFENSE);
        }
        for code in &self.variety {
            put(owner, *code, loc::DECK, 0, pos::FACEDOWN_DEFENSE);
        }
        for _ in 0..3 {
            put(owner, FILLER, loc::DECK, 0, pos::FACEDOWN_DEFENSE);
            put(owner, FILLER, loc::HAND, 0, pos::FACEDOWN_DEFENSE);
            put(owner, FILLER, loc::GRAVE, 0, pos::FACEUP_ATTACK);
        }
        // The other side: plain monsters, a Deck, a hand, one Set card.
        for _ in 0..10 {
            put(other, FILLER, loc::DECK, 0, pos::FACEDOWN_DEFENSE);
        }
        for _ in 0..2 {
            put(other, FILLER, loc::HAND, 0, pos::FACEDOWN_DEFENSE);
        }
        let mut target = CardRef { controller: owner, location: Location::MonsterZone, sequence: SUBJECT_ZONE };
        match scenario {
            Scenario::Attacked { attacker: profile, count, .. } => {
                self.core.define(ATTACKER, attacker(profile));
                for sequence in 0..count as u32 {
                    put(other, ATTACKER, loc::MZONE, sequence, pos::FACEUP_ATTACK);
                }
                put(other, JAR_OF_GREED, loc::SZONE, 4, pos::FACEDOWN_DEFENSE);
            }
            Scenario::Strike { defense, attribute, stats } => {
                self.core.define(WEAKLING, plain(stats, stats, 4, attribute));
                let position = if defense { pos::FACEUP_DEFENSE } else { pos::FACEUP_ATTACK };
                put(other, WEAKLING, loc::MZONE, 2, position);
                put(other, FILLER, loc::MZONE, 3, pos::FACEUP_ATTACK);
                put(other, JAR_OF_GREED, loc::SZONE, 4, pos::FACEDOWN_DEFENSE);
                target = CardRef { controller: other, location: Location::MonsterZone, sequence: 2 };
            }
            Scenario::Effect { card, times } => {
                put(other, FILLER, loc::MZONE, 3, pos::FACEUP_ATTACK);
                put(other, FILLER, loc::MZONE, 4, pos::FACEUP_ATTACK);
                let tool = self.core.printed(card).ok_or_else(|| format!("unknown card {card}"))?.kind;
                // A monster with a free effect is used again; anything else needs a copy each time.
                let copies = if card == CANNON_SOLDIER || card == BRIONAC { 1 } else { times as u32 };
                for sequence in 0..copies {
                    if tool & 0x1 != 0 {
                        put(other, card, loc::MZONE, sequence, pos::FACEUP_ATTACK);
                    } else if tool & 0x4 != 0 {
                        put(other, card, loc::SZONE, sequence, pos::FACEDOWN_DEFENSE);
                    } else {
                        put(other, card, loc::HAND, 0, pos::FACEDOWN_DEFENSE);
                    }
                }
                if card != JAR_OF_GREED {
                    put(other, JAR_OF_GREED, loc::SZONE, 4, pos::FACEDOWN_DEFENSE);
                }
            }
        }
        let profile = match scenario {
            Scenario::Attacked { attacker, .. } => attacker,
            _ => Attacker::BASE,
        };
        let attacker = card_data(ATTACKER, attacker(profile));
        let weakling = match scenario {
            Scenario::Strike { attribute, stats, .. } => card_data(WEAKLING, plain(stats, stats, 4, attribute)),
            _ => card_data(WEAKLING, plain(0, 0, 4, attribute::EARTH)),
        };
        let db = Arc::new(ProbeCards { cards: self.cards.clone(), attacker, weakling });
        let shared = Arc::new(Mutex::new(Shared::default()));
        let actor: Box<dyn Policy> = Box::new(Actor {
            scenario,
            target,
            entered_battle: false,
            attacked: HashSet::new(),
            activations: 0,
            uses: striking.then_some(subject),
            used: HashMap::new(),
            db: db.clone(),
            shared: shared.clone(),
        });
        let passive: Box<dyn Policy> =
            if striking { Box::new(Bystander) } else { Box::new(Owner { me: owner, subject, used: HashMap::new() }) };
        let mut seats = [Seat::new(actor, db.clone(), Some(0)), Seat::new(passive, db, Some(1))];
        let mut recorder = Recorder {
            scenario,
            subject,
            owner,
            slot: Some((owner, SUBJECT_ZONE)),
            counterpart: None,
            links: Vec::new(),
            tokens: self.tokens.clone(),
            turns: 0,
            board: Vec::new(),
            seen: Seen::default(),
        };
        let monsters = self.core.stage(&cards, subject as u64, &mut seats, 200, &mut |step| recorder.step(step))?;
        let mut seen = recorder.seen.clone();
        seen.after = recorder.subject_in(&monsters);
        let shared = shared.lock().unwrap();
        seen.notes.extend(shared.notes.iter().copied());
        seen.predicted = shared.predicted.clone();
        Ok(seen)
    }
}

/// Everything the probes showed about one monster.
#[derive(Clone, Debug)]
pub struct Examined {
    pub code: u32,
    /// Three attacks by the plain attacker: on the subject in Attack
    /// Position, and in Defense Position.
    pub attacked: [Seen; 2],
    /// One attack by other attackers, per position of the subject.
    pub variants: [Vec<(Attacker, Seen)>; 2],
    /// One attack on the subject in Defense Position by a monster too weak
    /// to destroy it.
    pub under: Option<Seen>,
    /// Each tool card used twice.
    pub effects: Vec<(u32, Seen)>,
    /// The subject attacks a 0/0 monster in Attack Position, and in Defense.
    pub strike: [Seen; 2],
    /// The same in Attack Position against the other Attributes, when the
    /// subject's attack did more than battle.
    pub strike_attributes: Vec<(u32, Seen)>,
    /// ...and against a 4000/4000 monster.
    pub strike_strong: Option<Seen>,
}

/// The cards aimed at the subject, in the order they are tried.
pub const TOOLS: [u32; 12] = [
    SMASHING_GROUND,
    SOUL_TAKER,
    BOOK_OF_MOON,
    DIAN_KETO,
    OOKAZI,
    RAIGEKI_BREAK,
    COMPULSORY_EVACUATION_DEVICE,
    JAR_OF_GREED,
    WABOKU,
    EXILED_FORCE,
    BRIONAC,
    CANNON_SOLDIER,
];

/// What matters when two attackers are compared: did the same things happen?
pub fn shape(seen: &Seen) -> (bool, bool, bool, Fate, Fate, bool) {
    match seen.acts.first() {
        Some(a) => (a.at_subject, a.negated, a.calc.is_some(), a.subject, a.counterpart, a.before_calc),
        None => (false, false, false, Fate::Stays, Fate::Stays, false),
    }
}

impl Lab {
    /// Every probe of one monster.  Attackers other than the plain one are
    /// tried once each: every other Attribute, a lower and a higher Level,
    /// and one barely strong enough.  Where one of them fares differently,
    /// the Levels or ATK values in between are tried too, to find where the
    /// difference starts.
    pub fn examine(&mut self, code: u32) -> Result<Examined> {
        let base = Attacker::BASE;
        let mut attacked = [Seen::default(), Seen::default()];
        let mut variants = [Vec::new(), Vec::new()];
        let mut under = None;
        for (p, defense) in [false, true].into_iter().enumerate() {
            attacked[p] = self.run(code, Scenario::Attacked { defense, attacker: base, count: 3 })?;
            let Some(before) = attacked[p].before else { continue };
            let once = |lab: &mut Lab, attacker: Attacker| -> Result<Seen> {
                lab.run(code, Scenario::Attacked { defense, attacker, count: 1 })
            };
            // The plain attacker alone first: what the others are compared with.
            let mut tried: Vec<(Attacker, Seen)> = vec![(base, once(self, base)?)];
            let first = shape(&tried[0].1);
            for attribute in attribute::ALL.into_iter().filter(|a| *a != base.attribute) {
                let attacker = Attacker { attribute, ..base };
                tried.push((attacker, once(self, attacker)?));
            }
            for race in race::OTHERS {
                let attacker = Attacker { race, ..base };
                tried.push((attacker, once(self, attacker)?));
            }
            let levels: Vec<(u32, Seen)> = [3u32, 8]
                .into_iter()
                .map(|level| once(self, Attacker { level, ..base }).map(|seen| (level, seen)))
                .collect::<Result<_>>()?;
            let by_level = levels.iter().any(|(_, seen)| shape(seen) != first);
            for (level, seen) in levels {
                tried.push((Attacker { level, ..base }, seen));
            }
            if by_level {
                for level in (1..=12).filter(|l| ![3, 4, 8].contains(l)) {
                    let attacker = Attacker { level, ..base };
                    tried.push((attacker, once(self, attacker)?));
                }
            }
            if defense {
                let attacker = Attacker { attack: (before.defense - 100).max(0), ..base };
                under = Some(once(self, attacker)?);
            }
            // The weakest attacker that still beats what is on the board.
            let stat = if defense { before.defense } else { before.attack };
            let close = (stat.max(0) / 100 + 1) * 100;
            if close < base.attack {
                let weak = Attacker { attack: close, ..base };
                let seen = once(self, weak)?;
                let differs = shape(&seen) != first;
                tried.push((weak, seen));
                if differs {
                    // Bisect for the ATK where the stronger attacker's result starts.
                    let (mut low, mut high) = (close / 100, base.attack / 100);
                    while high - low > 1 {
                        let middle = (low + high) / 2;
                        let attacker = Attacker { attack: middle * 100, ..base };
                        let seen = once(self, attacker)?;
                        if shape(&seen) == first {
                            high = middle;
                        } else {
                            low = middle;
                        }
                        tried.push((attacker, seen));
                    }
                }
            }
            variants[p] = tried;
        }
        let mut effects = Vec::new();
        for card in TOOLS {
            effects.push((card, self.run(code, Scenario::Effect { card, times: 2 })?));
        }
        let earth = attribute::EARTH;
        let strike = [
            self.run(code, Scenario::Strike { defense: false, attribute: earth, stats: 0 })?,
            self.run(code, Scenario::Strike { defense: true, attribute: earth, stats: 0 })?,
        ];
        let mut strike_attributes = Vec::new();
        let mut strike_strong = None;
        if strike[0].acts.first().is_some_and(|a| !matches!(a.counterpart, Fate::Battle | Fate::Stays)) {
            for attribute in attribute::ALL.into_iter().filter(|a| *a != earth) {
                strike_attributes.push((attribute, self.run(code, Scenario::Strike { defense: false, attribute, stats: 0 })?));
            }
            strike_strong = Some(self.run(code, Scenario::Strike { defense: false, attribute: earth, stats: 4000 })?);
        }
        Ok(Examined { code, attacked, variants, under, effects, strike, strike_attributes, strike_strong })
    }
}

// ---- JSON, for looking at what the probes saw -------------------------------

use serde_json::{json, Value};

fn cards_json(cards: &Cards) -> Value {
    json!([cards.monsters, cards.tokens, cards.spell_traps, cards.hand])
}

fn act_json(act: &Act) -> Value {
    json!({
        "at_subject": act.at_subject, "direct": act.direct, "negated": act.negated,
        "calc": act.calc.map(|c| json!([c.attacker_atk, c.target_atk, c.target_def, c.target_defending])),
        "visible": [json!(act.attacker_visible), json!(act.target_visible)],
        "targeted": act.targeted, "countered": act.countered,
        "subject": format!("{:?}", act.subject), "counterpart": format!("{:?}", act.counterpart),
        "before_calc": act.before_calc, "switched": act.switched,
        "damage": act.damage, "paid": act.paid,
        "gained": [cards_json(&act.gained[0]), cards_json(&act.gained[1])],
        "lost": [cards_json(&act.lost[0]), cards_json(&act.lost[1])],
        "spent": [cards_json(&act.spent[0]), cards_json(&act.spent[1])],
        "effects": act.effects,
    })
}

fn monster_json(monster: &Option<Monster>) -> Value {
    monster.map_or(Value::Null, |m| json!({"position": m.position, "attack": m.attack, "defense": m.defense}))
}

pub fn seen_json(seen: &Seen) -> Value {
    json!({
        "before": monster_json(&seen.before),
        "after": monster_json(&seen.after),
        "acts": seen.acts.iter().map(act_json).collect::<Vec<_>>(),
        "notes": seen.notes,
        "predicted": seen.predicted.iter().map(|o| format!("{o:?}")).collect::<Vec<_>>(),
    })
}

pub fn examined_json(e: &Examined) -> Value {
    let attacker = |a: &Attacker| json!([a.attack, a.level, a.attribute, a.race]);
    let variants = |list: &Vec<(Attacker, Seen)>| -> Vec<Value> {
        list.iter().map(|(a, seen)| json!({"attacker": attacker(a), "seen": seen_json(seen)})).collect()
    };
    json!({
        "code": e.code,
        "attacked": [seen_json(&e.attacked[0]), seen_json(&e.attacked[1])],
        "variants": [variants(&e.variants[0]), variants(&e.variants[1])],
        "under": e.under.as_ref().map(seen_json),
        "effects": e.effects.iter().map(|(card, seen)| json!({"card": card, "seen": seen_json(seen)})).collect::<Vec<_>>(),
        "strike": [seen_json(&e.strike[0]), seen_json(&e.strike[1])],
        "strike_attributes": e.strike_attributes.iter().map(|(a, seen)| json!({"attribute": a, "seen": seen_json(seen)})).collect::<Vec<_>>(),
        "strike_strong": e.strike_strong.as_ref().map(seen_json),
    })
}
