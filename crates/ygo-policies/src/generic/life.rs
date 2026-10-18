//! Public LP transactions, keyed by the exact printed activation or option.
//!
//! Sources are the corresponding `vendor/CardScripts/official/c<code>.lua`
//! files. Counts use public pile sizes/field occupancy and our visible GY.
//! This is a forecast against the observed board, not a claim that an unseen
//! response or an untracked, already-resolved turn effect cannot intervene.
use crate::agent::{Response, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, DecisionKind, Location};

const BARREL: u32 = 27053506;
const DESSERTS: u32 = 24068492;
const POISON: u32 = 8842266;
const STRIKE: u32 = 91623717;
const FORTUNE: u32 = 98444741;
const JAR: u32 = 83968380;
const LEGACY: u32 = 30461781;
const RECKLESS: u32 = 37576645;
const EXPLOSION: u32 = 32723153;
const OOKAZI: u32 = 19523799;
const THIEF: u32 = 45311864;
const FIRE: u32 = 46918794;
const GIFT: u32 = 39526584;
const RAIN: u32 = 66719324;
const UPSTART: u32 = 70368879;
const SOUL: u32 = 81510157;
const DIAN: u32 = 84257639;
const NURSE: u32 = 67316075;
const SIMOCHI: u32 = 40633297;
const DECREE: u32 = 51452091;
const JINZO: u32 = 77585513;
const BLACK_WINGED: u32 = 9012916;
const PRIME_MATERIAL: u32 = 12298909;
const DRACO_EQUISTE: u32 = 14017402;

pub const SUPPORTED_CODES: &[u32] = &[
    BARREL,
    DESSERTS,
    POISON,
    STRIKE,
    FORTUNE,
    JAR,
    LEGACY,
    RECKLESS,
    EXPLOSION,
    OOKAZI,
    THIEF,
    FIRE,
    GIFT,
    RAIN,
    UPSTART,
    SOUL,
    DIAN,
    NURSE,
    SIMOCHI,
    DECREE,
    JINZO,
    BLACK_WINGED,
    PRIME_MATERIAL,
    DRACO_EQUISTE,
    crate::knowledge::SKILL_DRAIN,
];

#[derive(Clone, Copy)]
enum Recipient {
    Controller,
    Opponent,
}

#[derive(Clone, Copy)]
enum Amount {
    Fixed(i32),
    OpponentMonsters(i32),
    OpponentHandAndField(i32),
    OwnGraveSpells(i32),
    ChainLink(i32),
}

#[derive(Clone, Copy)]
enum Change {
    Damage(Recipient, Amount),
    Recover(Recipient, i32),
}

#[derive(Clone, Copy)]
struct Transaction {
    changes: &'static [Change],
    draws: u32,
}

fn transaction(code: u32, poison_heal: bool) -> Option<Transaction> {
    use Amount::*;
    use Change::*;
    use Recipient::*;
    let changes: &'static [Change] = match code {
        BARREL => &[Damage(Opponent, OpponentHandAndField(200))],
        DESSERTS => &[Damage(Opponent, OpponentMonsters(500))],
        POISON if poison_heal => &[Recover(Controller, 1200)],
        POISON | OOKAZI => &[Damage(Opponent, Fixed(800))],
        STRIKE => &[Damage(Opponent, ChainLink(400))],
        FORTUNE => &[],
        EXPLOSION => &[Damage(Opponent, OwnGraveSpells(200))],
        // Both scripts defer their paired LP changes until RDComplete.
        THIEF => &[Damage(Opponent, Fixed(500)), Recover(Controller, 500)],
        FIRE => &[
            Damage(Opponent, Fixed(1000)),
            Damage(Controller, Fixed(500)),
        ],
        GIFT => &[Recover(Opponent, 3000)],
        RAIN => &[Recover(Controller, 1000), Recover(Opponent, 1000)],
        UPSTART => &[Recover(Opponent, 1000)],
        DIAN => &[Recover(Controller, 1000)],
        // Soul Taker's recovery requires successful target destruction. Its
        // removal stays with the existing targeting rules; only setup below
        // knows about the conditional recovery, never a guaranteed LP payoff.
        _ => return None,
    };
    Some(Transaction {
        changes,
        draws: match code {
            FORTUNE => 2,
            UPSTART => 1,
            _ => 0,
        },
    })
}

/// Whether the existing, source-backed LP transaction facts offer damage to
/// the opponent. This describes a printed option, not current legality, net
/// payoff, or an inferred deck strategy. Callers canonicalize alternate art.
pub fn offers_opponent_damage(code: u32) -> bool {
    transaction(code, false).is_some_and(|transaction| {
        transaction.changes.iter().any(|change| {
            matches!(change, Change::Damage(Recipient::Opponent, _))
        })
    })
}

fn exact_activation(t: &Turn, choice: &Choice) -> Option<u32> {
    let at = choice.at()?;
    let code = t.ctx.canonical(choice.code()?);
    (choice.kind == ChoiceKind::Activate
        && choice.description == 0
        && at.controller == t.ctx.me
        && matches!(at.location, Location::Hand | Location::SpellTrapZone)
        && !matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. })
        && (transaction(code, false).is_some() || matches!(code, SIMOCHI | SOUL)))
    .then_some(code)
}

fn pending(t: &Turn, card: &CardView) -> bool {
    t.ctx.obs.chain.iter().any(|link| {
        link.source == card.at
            && card
                .code
                .is_some_and(|code| t.ctx.canonical(code) == t.ctx.canonical(link.code))
    })
}

fn face_up_source(t: &Turn, card: &CardView, code: u32, location: Location) -> bool {
    card.at.location == location && card.position.face_up
        && card.code.is_some_and(|candidate| t.ctx.canonical(candidate) == code)
        // A monster retains its continuous effects while its separate
        // activated effect is on the chain. Backrow installation must resolve.
        && (location != Location::SpellTrapZone || !pending(t, card))
}

fn present(t: &Turn, code: u32, location: Location) -> bool {
    t.ctx
        .obs
        .cards
        .iter()
        .any(|card| face_up_source(t, card, code, location))
}

fn drain_active(t: &Turn) -> bool {
    present(t, crate::knowledge::SKILL_DRAIN, Location::SpellTrapZone)
        && !present(t, DECREE, Location::SpellTrapZone)
}

fn ambiguous_negation(t: &Turn) -> bool {
    // Jinzo and Skill Drain can suppress each other depending on the order
    // they became effective. A bare board snapshot does not establish it.
    drain_active(t) && present(t, JINZO, Location::MonsterZone)
}

fn traps_negated(t: &Turn) -> bool {
    t.ctx.obs.cards.iter().any(|card| {
        face_up_source(t, card, DECREE, Location::SpellTrapZone)
            || !drain_active(t) && face_up_source(t, card, JINZO, Location::MonsterZone)
    })
}

fn stopped(t: &Turn, code: u32, origin: Option<Location>) -> bool {
    if ambiguous_negation(t) || t.ctx.data(code).is_trap() && traps_negated(t) {
        return true;
    }
    if drain_active(t) {
        return false;
    }
    // Use the shared negator facts, but with resolved Drain/Decree state. The
    // general Ctx presence check also counts a still-pending Skill Drain.
    // This particular copy may leave the hand. Another legal copy of the
    // same card in hand must not change the forecast for a set activation.
    let from_hand = origin == Some(Location::Hand);
    let our_hand = t
        .ctx
        .hand_size(t.ctx.me)
        .saturating_sub(u32::from(from_hand));
    t.ctx.monsters(t.ctx.opp).iter().any(|card| {
        if !card.position.face_up {
            return false;
        }
        let Some(negator_code) = card.code else {
            return false;
        };
        let facts = crate::knowledge::facts(t.ctx.canonical(negator_code));
        let applies = match facts.needs {
            crate::knowledge::Needs::Nothing => true,
            crate::knowledge::Needs::EmptyHand => t.ctx.hand_size(card.at.controller) == 0,
            crate::knowledge::Needs::EqualHands => our_hand == t.ctx.hand_size(t.ctx.opp),
        };
        applies && facts.negates_any & t.ctx.kind_of(code) != 0
    })
}

fn reversal(t: &Turn, recipient: u8) -> bool {
    t.ctx.obs.cards.iter().any(|card| {
        card.at.controller != recipient
            && (face_up_source(t, card, NURSE, Location::MonsterZone) && !drain_active(t)
                || face_up_source(t, card, SIMOCHI, Location::SpellTrapZone) && !traps_negated(t))
    })
}

fn amount(t: &Turn, amount: Amount, resolving: bool) -> Option<i32> {
    let ctx = t.ctx;
    let (count, multiplier) = match amount {
        Amount::Fixed(value) => return Some(value),
        Amount::OpponentMonsters(value) => (ctx.monsters(ctx.opp).len(), value),
        Amount::OpponentHandAndField(value) => (
            ctx.hand_size(ctx.opp) as usize
                + ctx
                    .obs
                    .cards
                    .iter()
                    .filter(|card| card.at.controller == ctx.opp && card.at.location.is_field())
                    .count(),
            value,
        ),
        Amount::OwnGraveSpells(value) => (
            ctx.obs
                .cards
                .iter()
                .filter(|card| {
                    card.at.controller == ctx.me
                        && card.at.location == Location::Graveyard
                        && card.code.is_some_and(|code| ctx.data(code).is_spell())
                })
                .count(),
            value,
        ),
        Amount::ChainLink(value) => {
            if !ctx.obs.chain_known || resolving {
                return None;
            }
            (ctx.obs.chain.len() + 1, value)
        }
    };
    Some(
        i32::try_from(count)
            .unwrap_or(i32::MAX)
            .saturating_mul(multiplier),
    )
}

// Only public, continuously present damage modifiers are used. In particular,
// seeing Rainbow Life in a GY does not prove its turn effect is still active.
fn damage(t: &Turn, recipient: u8, value: i32) -> Option<i32> {
    if value <= 0 {
        return Some(0);
    }
    if ambiguous_negation(t) {
        return None;
    }
    if drain_active(t) {
        return Some(value);
    }
    let live = |card: &CardView, code| face_up_source(t, card, code, Location::MonsterZone);
    if t.ctx
        .obs
        .cards
        .iter()
        .any(|card| live(card, PRIME_MATERIAL))
    {
        // Reverse-damage/reverse-recovery interactions have native ordering.
        // Do not claim lethal without modeling that full replacement stack.
        return None;
    }
    if t.ctx.obs.cards.iter().any(|card| {
        card.at.controller == recipient && live(card, DRACO_EQUISTE) && card.position.attack
    }) {
        return None;
    }
    if t.ctx
        .obs
        .cards
        .iter()
        .any(|card| card.at.controller == recipient && live(card, BLACK_WINGED))
    {
        return Some(0);
    }
    Some(value)
}

#[derive(Clone, Copy, Debug)]
struct Forecast {
    loss: [i32; 2],
    draws: u32,
}

impl Forecast {
    fn survives(self, ctx: &Ctx) -> bool {
        self.loss[ctx.me as usize] < ctx.my_lp()
    }
    fn lethal(self, ctx: &Ctx) -> bool {
        self.survives(ctx) && self.loss[ctx.opp as usize] >= ctx.opp_lp()
    }
    fn useful(self, ctx: &Ctx) -> bool {
        self.survives(ctx)
            && (self.loss[ctx.opp as usize] > 0
                || self.draws > 0
                || self.loss[ctx.me as usize] < 0 && self.loss[ctx.opp as usize] == 0)
    }
}

/// These are payoffs already offered by the engine, never a prediction that
/// another set card will become activatable after extending the chain.
fn offered_chain_payoff(t: &Turn, code: u32) -> bool {
    let minimum = match code {
        STRIKE => 1,
        FORTUNE => 3,
        _ => return false,
    };
    let chain = &t.ctx.obs.chain;
    t.ctx.obs.chain_known
        && chain.len() >= minimum
        // CheckChainUniqueness compares current runtime names. Exact repeated
        // observed passcodes are a conservative rejection; do not canonicalize
        // aliases or pretend this models every runtime name change. The actual
        // legal offer remains authoritative. The candidate is not yet a link.
        && chain.iter().enumerate().all(|(index, link)| {
            !chain[..index].iter().any(|earlier| earlier.code == link.code)
        })
}

/// Fortune resolves above these draws, so all must fit in the remaining deck.
/// Legacy's selected mode is absent from ChainLink; reserve its maximum two.
/// These facts grant no new activation permission to the pending draw cards.
fn pending_known_draws(t: &Turn) -> u32 {
    t.ctx.obs.chain.iter().filter(|link| link.controller == t.ctx.me).fold(0u32, |draws, link| {
        draws.saturating_add(match t.ctx.canonical(link.code) {
            JAR | UPSTART => 1,
            LEGACY | RECKLESS | FORTUNE => 2,
            _ => 0,
        })
    })
}

fn forecast(t: &Turn, code: u32, poison_heal: bool, origin: Option<Location>) -> Option<Forecast> {
    let transaction = transaction(code, poison_heal)?;
    if stopped(t, code, origin) || transaction.draws > t.ctx.deck_size(t.ctx.me) {
        return None;
    }
    if matches!(code, STRIKE | FORTUNE) && !offered_chain_payoff(t, code) {
        return None;
    }
    if code == FORTUNE
        && transaction.draws.saturating_add(pending_known_draws(t)) > t.ctx.deck_size(t.ctx.me)
    {
        return None;
    }
    let mut result = Forecast {
        loss: [0; 2],
        draws: transaction.draws,
    };
    for change in transaction.changes {
        let (who, value, recovering) = match *change {
            Change::Damage(who, value) => (who, amount(t, value, origin.is_none())?, false),
            Change::Recover(who, value) => (who, value, true),
        };
        let recipient = match who {
            Recipient::Controller => t.ctx.me,
            Recipient::Opponent => t.ctx.opp,
        };
        let loss = if recovering && !reversal(t, recipient) {
            -value
        } else {
            damage(t, recipient, value)?
        };
        result.loss[recipient as usize] = result.loss[recipient as usize].saturating_add(loss);
    }
    Some(result)
}

fn prefers_poison_heal(t: &Turn, origin: Option<Location>) -> bool {
    let burn = forecast(t, POISON, false, origin);
    if burn.is_some_and(|result| result.useful(&t.ctx)) {
        return false;
    }
    forecast(t, POISON, true, origin).is_some_and(|result| result.useful(&t.ctx))
}

fn recovery_reaches(code: u32, controller: u8, recipient: u8) -> bool {
    match code {
        GIFT | UPSTART | SOUL => recipient != controller,
        RAIN => true,
        THIEF | DIAN => recipient == controller,
        _ => false, // Poison's chosen mode is not present in ChainLink.
    }
}

fn pending_recovery(t: &Turn) -> bool {
    t.ctx.obs.chain_known
        && t.ctx.obs.chain.iter().any(|link| {
            link.source.location == Location::SpellTrapZone
                && recovery_reaches(t.ctx.canonical(link.code), link.controller, t.ctx.opp)
        })
}

fn simochi_useful(t: &Turn, origin: Option<Location>) -> bool {
    if stopped(t, SIMOCHI, origin) || reversal(t, t.ctx.opp)
        || damage(t, t.ctx.opp, 1) != Some(1)
        || t.ctx.obs.chain.iter().any(|link| link.controller == t.ctx.me
            && t.ctx.canonical(link.code) == SIMOCHI)
        // A lower Decree can resolve after Simochi but before the recovery.
        || t.ctx.obs.chain.iter().any(|link| t.ctx.canonical(link.code) == DECREE)
    {
        return false;
    }
    if pending_recovery(t) {
        return true;
    }
    if t.own_top() {
        return false;
    }
    t.ctx.obs.cards.iter().any(|card| {
        card.at.controller == t.ctx.me
            && (card.at.location == Location::Hand
                || card.at.location == Location::SpellTrapZone && !card.position.face_up)
            && card
                .code
                .is_some_and(|code| recovery_reaches(t.ctx.canonical(code), t.ctx.me, t.ctx.opp))
    })
}

/// Exact activation veto/permission. `None` leaves unrelated effects alone.
pub(super) fn activate(t: &Turn, choice: &Choice) -> Option<bool> {
    let code = exact_activation(t, choice)?;
    let origin = choice.at().map(|at| at.location);
    if code == SOUL {
        return None;
    }
    if code == SIMOCHI {
        return Some(simochi_useful(t, origin));
    }
    Some(
        forecast(
            t,
            code,
            code == POISON && prefers_poison_heal(t, origin),
            origin,
        )
        .is_some_and(|result| result.useful(&t.ctx)),
    )
}

/// Our own top link is crossed for an independently lethal LP change, an
/// already-offered unique-chain payoff, or pending recovery reversal setup.
pub(super) fn chain(t: &Turn, choice: &Choice) -> Option<Response> {
    let code = exact_activation(t, choice)?;
    let origin = choice.at().map(|at| at.location);
    if code == SOUL {
        return None;
    }
    if code == SIMOCHI {
        return Some(if !simochi_useful(t, origin) {
            Response::no()
        } else {
            Response::new(if pending_recovery(t) { 110.0 } else { 35.0 })
        });
    }
    let result = forecast(
        t,
        code,
        code == POISON && prefers_poison_heal(t, origin),
        origin,
    );
    Some(match result {
        Some(result) if result.lethal(&t.ctx) => Response::new(150.0),
        Some(result) if t.own_top() && matches!(code, STRIKE | FORTUNE) && result.useful(&t.ctx) => Response::new(110.0),
        Some(result) if !t.own_top() && result.useful(&t.ctx) => Response::new(35.0),
        _ => Response::no(),
    })
}

pub(super) fn activate_before_development(t: &mut Turn) -> Option<usize> {
    let lethal = t
        .choices()
        .find(|(index, choice)| {
            t.fresh(*index)
                && exact_activation(t, choice).is_some_and(|code| {
                    forecast(t, code, false, choice.at().map(|at| at.location))
                        .is_some_and(|result| result.lethal(&t.ctx))
                })
        })
        .map(|(index, _)| index);
    let setup = || {
        t.choices()
            .find(|(index, choice)| {
                t.fresh(*index)
                    && exact_activation(t, choice) == Some(SIMOCHI)
                    && simochi_useful(t, choice.at().map(|at| at.location))
            })
            .map(|(index, _)| index)
    };
    lethal.or_else(setup).and_then(|index| t.pick(index))
}

pub(super) fn option(t: &Turn) -> Option<usize> {
    if t.decision.kind != DecisionKind::Option
        || t.memory.last_activated.map(|code| t.ctx.canonical(code)) != Some(POISON)
    {
        return None;
    }
    // Native Auxiliary.Stringid uses a 20-bit effect slot, not code * 16.
    let description = ((POISON as u64) << 20) + u64::from(!prefers_poison_heal(t, None));
    t.choices()
        .find(|(_, choice)| choice.kind == ChoiceKind::Option && choice.description == description)
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{types, CardData, MemoryCards};
    use crate::model::{CardRef, ChainLink, Decision, Hint, Member, Observation, Phase, Position};

    #[test]
    fn opponent_damage_fact_describes_printed_options_without_conversion_inference() {
        for code in [BARREL, DESSERTS, POISON, STRIKE, EXPLOSION, OOKAZI, THIEF, FIRE] {
            assert!(offers_opponent_damage(code), "missing existing damage option {code}");
        }
        for code in [FORTUNE, GIFT, RAIN, UPSTART, SOUL, DIAN, NURSE, SIMOCHI, 102380, 0] {
            assert!(!offers_opponent_damage(code), "invented damage option {code}");
        }
    }

    fn db() -> MemoryCards {
        MemoryCards(
            SUPPORTED_CODES
                .iter()
                .copied()
                .chain([
                    crate::knowledge::SKILL_DRAIN,
                    crate::knowledge::FABLED_UNICORE,
                ])
                .map(|code| {
                    let kind = match code {
                        NURSE
                        | JINZO
                        | BLACK_WINGED
                        | PRIME_MATERIAL
                        | DRACO_EQUISTE
                        | crate::knowledge::FABLED_UNICORE => types::MONSTER | types::EFFECT,
                        BARREL
                        | DESSERTS
                        | EXPLOSION
                        | FORTUNE
                        | JAR
                        | LEGACY
                        | RECKLESS
                        | GIFT
                        | SIMOCHI
                        | DECREE
                        | crate::knowledge::SKILL_DRAIN => types::TRAP,
                        _ => types::SPELL,
                    };
                    (
                        code,
                        CardData {
                            code,
                            kind,
                            ..CardData::default()
                        },
                    )
                })
                .collect(),
        )
    }

    fn card(code: u32, controller: u8, location: Location, sequence: u32) -> CardView {
        CardView {
            at: CardRef {
                controller,
                location,
                sequence,
            },
            code: Some(code),
            position: Position::FACE_UP_ATTACK,
            attack: 1000,
            defense: 1000,
            level: 4,
            can_attack: false,
            battles: 0,
            counters: 0,
            coin_effect: None,
        }
    }

    fn observation(me: u8) -> Observation {
        Observation {
            me,
            turn: 21,
            turn_player: Some(me),
            phase: Some(Phase::Main1),
            life_points: [8000; 2],
            cards: vec![],
            pile_sizes: vec![(0, Location::Deck, 20), (1, Location::Deck, 20)],
            chain: vec![],
            battle_attacker: None,
            battle_target: None,
            event_cards: vec![],
            summon_used: false,
            chain_known: true,
            can_attack_known: true,
            coin_toss: None,
        }
    }

    fn activation(code: u32, me: u8, location: Location) -> Choice {
        Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member {
                at: CardRef {
                    controller: me,
                    location,
                    sequence: 0,
                },
                code: Some(code),
                value: 0,
                required: false,
            }),
            members: vec![],
            description: 0,
            place: None,
        }
    }

    fn decision(choices: Vec<Choice>) -> Decision {
        Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: None,
            choices,
        }
    }

    fn with<R>(obs: &Observation, decision: &Decision, f: impl FnOnce(&mut Turn) -> R) -> R {
        let db = db();
        let mut memory = Memory::default();
        f(&mut Turn {
            ctx: Ctx::new(obs, &db),
            decision,
            memory: &mut memory,
        })
    }

    fn link(code: u32, controller: u8, sequence: u32) -> ChainLink {
        ChainLink {
            code,
            controller,
            source: CardRef {
                controller,
                location: Location::SpellTrapZone,
                sequence,
            },
            targets: vec![],
        }
    }

    fn payoff_prompt(me: u8, code: u32) -> (Observation, Decision) {
        let mut obs = observation(me);
        let mut source = card(code, me, Location::SpellTrapZone, 0);
        source.position = Position::FACE_DOWN_DEFENSE;
        obs.cards.push(source);
        obs.chain = if code == FORTUNE {
            vec![link(OOKAZI, 1 - me, 1), link(POISON, 1 - me, 2), link(RAIN, me, 3)]
        } else {
            vec![link(OOKAZI, me, 1)]
        };
        let choice = activation(code, me, Location::SpellTrapZone);
        let mut pass = choice.clone();
        pass.kind = ChoiceKind::Pass;
        pass.card = None;
        let mut prompt = decision(vec![choice, pass]);
        prompt.kind = DecisionKind::Chain { forced: false, triggers: false };
        (obs, prompt)
    }

    fn set_deck(obs: &mut Observation, player: u8, count: u32) {
        obs.pile_sizes.retain(|&(owner, location, _)| owner != player || location != Location::Deck);
        obs.pile_sizes.push((player, Location::Deck, count));
    }

    #[test]
    fn offered_unique_chain_payoffs_cross_own_top_in_actual_generic_agent() {
        use crate::agent::{Agent, Policy};
        use std::sync::Arc;
        for me in [0, 1] {
            for code in [STRIKE, FORTUNE] {
                let (obs, prompt) = payoff_prompt(me, code);
                with(&obs, &prompt, |t| {
                    assert_eq!(activate(t, &prompt.choices[0]), Some(true));
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 110.0);
                });
                let mut pilot = Agent::seeded(super::super::Generic, Arc::new(db()), 1);
                assert_eq!(pilot.choose(&obs, &prompt), 0, "seat {me}, source {code}");
                let mut opposing_top = obs.clone();
                let last = opposing_top.chain.last_mut().unwrap();
                last.controller = 1 - me;
                last.source.controller = 1 - me;
                with(&opposing_top, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 35.0,
                        "ordinary response priority remains below negation");
                });
            }
        }
    }

    #[test]
    fn chain_payoffs_require_known_threshold_and_no_observed_duplicate_links() {
        for me in [0, 1] {
            for code in [STRIKE, FORTUNE] {
                let (mut obs, prompt) = payoff_prompt(me, code);
                obs.chain_known = false;
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
                });
                obs.chain_known = true;
                obs.chain.pop();
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
                });
                obs.chain = vec![link(OOKAZI, me, 1), link(OOKAZI, 1 - me, 2), link(RAIN, me, 3)];
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
                });
                // Native uniqueness checks the existing chain, before this
                // candidate is added. One existing copy alone is not a pair.
                obs.chain = if code == FORTUNE {
                    vec![link(code, me, 1), link(POISON, 1 - me, 2), link(RAIN, me, 3)]
                } else {
                    vec![link(code, me, 1)]
                };
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 110.0);
                });
            }
        }
    }

    #[test]
    fn fortune_reserves_deck_for_already_pending_known_draws() {
        for me in [0, 1] {
            for (pending, draws) in [(JAR, 1), (UPSTART, 1), (LEGACY, 2), (RECKLESS, 2), (FORTUNE, 2)] {
                let (mut obs, prompt) = payoff_prompt(me, FORTUNE);
                obs.chain[0] = link(pending, me, 1);
                set_deck(&mut obs, me, draws + 1);
                with(&obs, &prompt, |t| {
                    assert_eq!(activate(t, &prompt.choices[0]), Some(false));
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
                });
                set_deck(&mut obs, me, draws + 2);
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 110.0);
                });
                // These scripts draw for their own controller, so their
                // opponent-controlled copies do not consume our deck.
                obs.chain[0].controller = 1 - me;
                obs.chain[0].source.controller = 1 - me;
                set_deck(&mut obs, me, 2);
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 110.0);
                });
            }
            let (mut obs, prompt) = payoff_prompt(me, FORTUNE);
            obs.chain = vec![link(JAR, me, 1), link(LEGACY, me, 2), link(RECKLESS, me, 3)];
            for (remaining, allowed) in [(6, false), (7, true)] {
                set_deck(&mut obs, me, remaining);
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score > 100.0, allowed);
                });
            }
            set_deck(&mut obs, me, 1);
            with(&obs, &prompt, |t| {
                assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
            });
        }
    }

    #[test]
    fn offered_payoffs_keep_negation_and_damage_prevention_guards() {
        for me in [0, 1] {
            for (negator, location) in [(DECREE, Location::SpellTrapZone), (JINZO, Location::MonsterZone)] {
                let (mut obs, prompt) = payoff_prompt(me, FORTUNE);
                obs.cards.push(card(negator, 1 - me, location, 4));
                with(&obs, &prompt, |t| {
                    assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
                });
            }
            let (mut obs, prompt) = payoff_prompt(me, STRIKE);
            obs.cards.push(card(BLACK_WINGED, 1 - me, Location::MonsterZone, 4));
            with(&obs, &prompt, |t| {
                assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0);
            });
        }
    }

    #[test]
    fn direct_payoff_exception_matches_only_exact_offered_activation() {
        for code in [STRIKE, FORTUNE] {
            let (obs, original) = payoff_prompt(0, code);
            for location in [Location::Graveyard, Location::Banished, Location::MonsterZone] {
                let mut prompt = original.clone();
                prompt.choices[0].card.as_mut().unwrap().at.location = location;
                with(&obs, &prompt, |t| {
                    assert!(chain(t, &prompt.choices[0]).is_none());
                });
            }
            let mut prompt = original.clone();
            prompt.choices[0].description = (code as u64) << 20;
            with(&obs, &prompt, |t| assert!(chain(t, &prompt.choices[0]).is_none()));
            prompt = original.clone();
            prompt.choices[0].card.as_mut().unwrap().at.controller = 1;
            with(&obs, &prompt, |t| assert!(chain(t, &prompt.choices[0]).is_none()));
            prompt = original.clone();
            prompt.kind = DecisionKind::Chain { forced: false, triggers: true };
            with(&obs, &prompt, |t| assert!(chain(t, &prompt.choices[0]).is_none()));
        }
        let (obs, mut prompt) = payoff_prompt(0, STRIKE);
        prompt.choices[0].card.as_mut().unwrap().at.location = Location::Hand;
        with(&obs, &prompt, |t| assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 110.0));
        for unrelated in [JAR, LEGACY, RECKLESS] {
            prompt.choices[0].card.as_mut().unwrap().code = Some(unrelated);
            with(&obs, &prompt, |t| assert!(chain(t, &prompt.choices[0]).is_none()));
        }
        prompt.choices[0].card.as_mut().unwrap().code = Some(POISON);
        with(&obs, &prompt, |t| assert_eq!(chain(t, &prompt.choices[0]).unwrap().score, 0.0));
    }

    #[test]
    fn barrel_trace_496_lethal_and_564_response_both_seats() {
        // burn seed1754017719 seat0: trace496 has opp200LP, five monsters,
        // two backrow and zero hand; trace564 still offers Barrel over Cannon.
        for me in [0, 1] {
            let opp = 1 - me;
            let mut obs = observation(me);
            obs.life_points[opp as usize] = 200;
            obs.cards
                .extend((0..5).map(|i| card(0, opp, Location::MonsterZone, i)));
            obs.cards
                .extend((0..2).map(|i| card(0, opp, Location::SpellTrapZone, i)));
            let choice = activation(BARREL, me, Location::SpellTrapZone);
            let d = decision(vec![choice.clone()]);
            for lower in [None, Some(link(38992735, opp, 4)), Some(link(THIEF, me, 4))] {
                obs.chain = lower.into_iter().collect();
                with(&obs, &d, |t| {
                    assert_eq!(
                        forecast(t, BARREL, false, Some(Location::SpellTrapZone))
                            .unwrap()
                            .loss[opp as usize],
                        1400
                    );
                    assert_eq!(
                        chain(t, &choice).map(|response| response.score > 0.0),
                        Some(true)
                    );
                    assert_eq!(activate_before_development(t), Some(0));
                });
            }
            obs.life_points[opp as usize] = 8000;
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(false)
                )
            });
            obs.chain.clear();
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(true)
                )
            });
        }
    }

    #[test]
    fn barrel_uses_public_counts_never_opposing_hand_identity() {
        let choice = activation(BARREL, 0, Location::SpellTrapZone);
        let d = decision(vec![choice.clone()]);
        for hidden_code in [None, Some(NURSE), Some(PRIME_MATERIAL), Some(12345678)] {
            let mut obs = observation(0);
            obs.life_points[1] = 800;
            obs.pile_sizes.push((1, Location::Hand, 4));
            let mut hidden = card(0, 1, Location::Hand, 0);
            hidden.code = hidden_code;
            hidden.position = Position::FACE_DOWN_DEFENSE;
            obs.cards.push(hidden);
            with(&obs, &d, |t| {
                assert_eq!(
                    forecast(t, BARREL, false, Some(Location::SpellTrapZone))
                        .unwrap()
                        .loss,
                    [0, 800]
                );
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(true)
                );
            });
        }
    }

    #[test]
    fn simochi_before_recovery_and_above_pending_own_recovery() {
        // nurse seed1745706923 seat0 traces199/203,206/209: legal Simochi
        // is passed before and above Soul Taker/Rain, healing the opponent.
        for me in [0, 1] {
            let mut obs = observation(me);
            let rain = activation(RAIN, me, Location::Hand);
            let setup = activation(SIMOCHI, me, Location::SpellTrapZone);
            let d = decision(vec![rain.clone(), setup.clone()]);
            obs.cards.push(card(RAIN, me, Location::Hand, 0));
            with(&obs, &d, |t| {
                assert_eq!(activate(t, &rain), Some(false));
                assert_eq!(activate_before_development(t), Some(1));
            });
            obs.cards.clear();
            for source in [RAIN, SOUL, GIFT, UPSTART] {
                obs.chain = vec![link(source, me, 2)];
                with(&obs, &d, |t| {
                    assert_eq!(
                        chain(t, &setup).map(|response| response.score > 0.0),
                        Some(true)
                    )
                });
            }
            obs.chain = vec![link(RAIN, me, 2), link(DECREE, 1 - me, 4)];
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &setup).map(|response| response.score > 0.0),
                    Some(false)
                )
            });
        }
    }

    #[test]
    fn pending_simochi_is_not_active_and_redundant_copy_is_not_used() {
        for me in [0, 1] {
            let opp = 1 - me;
            let mut obs = observation(me);
            obs.life_points[opp as usize] = 3000;
            obs.cards
                .push(card(SIMOCHI, me, Location::SpellTrapZone, 0));
            obs.chain.push(link(SIMOCHI, me, 0));
            let gift = activation(GIFT, me, Location::SpellTrapZone);
            let setup = activation(SIMOCHI, me, Location::SpellTrapZone);
            let d = decision(vec![gift.clone(), setup.clone()]);
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &gift).map(|response| response.score > 0.0),
                    Some(false)
                );
                assert_eq!(
                    chain(t, &setup).map(|response| response.score > 0.0),
                    Some(false)
                );
            });
            obs.chain.clear();
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &gift).map(|response| response.score > 0.0),
                    Some(true)
                );
                assert_eq!(activate_before_development(t), Some(0));
                assert_eq!(
                    chain(t, &setup).map(|response| response.score > 0.0),
                    Some(false)
                );
            });
        }
    }

    #[test]
    fn recipient_reversal_is_directional_and_self_lethal_is_rejected() {
        for me in [0, 1] {
            let opp = 1 - me;
            let mut obs = observation(me);
            let rain = activation(RAIN, me, Location::Hand);
            let fire = activation(FIRE, me, Location::Hand);
            let d = decision(vec![rain.clone(), fire.clone()]);
            obs.cards.push(card(NURSE, me, Location::MonsterZone, 0));
            with(&obs, &d, |t| {
                let result = forecast(t, RAIN, false, Some(Location::SpellTrapZone)).unwrap();
                assert_eq!(result.loss[me as usize], -1000);
                assert_eq!(result.loss[opp as usize], 1000);
                assert_eq!(activate(t, &rain), Some(true));
            });
            obs.cards.push(card(NURSE, opp, Location::MonsterZone, 0));
            obs.life_points = [1000; 2];
            with(&obs, &d, |t| assert_eq!(activate(t, &rain), Some(false)));
            obs.cards.clear();
            obs.life_points[me as usize] = 500;
            with(&obs, &d, |t| {
                assert_eq!(activate(t, &fire), Some(false));
                assert_eq!(activate_before_development(t), None);
            });
            obs.life_points[me as usize] = 501;
            with(&obs, &d, |t| assert_eq!(activate(t, &fire), Some(true)));
        }
    }

    #[test]
    fn damage_modifiers_negation_and_facedown_sources_are_conservative() {
        let choice = activation(BARREL, 0, Location::SpellTrapZone);
        let d = decision(vec![choice.clone()]);
        for code in [BLACK_WINGED, PRIME_MATERIAL, DRACO_EQUISTE, JINZO, DECREE] {
            let mut obs = observation(0);
            obs.life_points[1] = 200;
            let location = if code == DECREE {
                Location::SpellTrapZone
            } else {
                Location::MonsterZone
            };
            obs.cards.push(card(code, 1, location, 0));
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(false),
                    "{code}"
                )
            });
            obs.cards[0].position = Position::FACE_DOWN_DEFENSE;
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(true),
                    "set {code}"
                )
            });
            obs.cards[0].position = Position::FACE_UP_ATTACK;
            if code != DECREE {
                obs.cards.push(card(
                    crate::knowledge::SKILL_DRAIN,
                    0,
                    Location::SpellTrapZone,
                    1,
                ));
                with(&obs, &d, |t| {
                    assert_eq!(
                        chain(t, &choice).map(|response| response.score > 0.0),
                        Some(code != JINZO),
                        "drained {code}"
                    )
                });
            }
        }
    }

    #[test]
    fn nurse_and_simochi_respect_their_own_negation_domains() {
        let gift = activation(GIFT, 0, Location::SpellTrapZone);
        let rain = activation(RAIN, 0, Location::Hand);
        let d = decision(vec![gift.clone(), rain.clone()]);
        let mut obs = observation(0);
        obs.cards.push(card(NURSE, 0, Location::MonsterZone, 0));
        obs.cards.push(card(
            crate::knowledge::SKILL_DRAIN,
            1,
            Location::SpellTrapZone,
            0,
        ));
        with(&obs, &d, |t| assert_eq!(activate(t, &gift), Some(false)));
        obs.cards[0] = card(SIMOCHI, 0, Location::SpellTrapZone, 0);
        with(&obs, &d, |t| assert_eq!(activate(t, &gift), Some(true)));
        obs.cards[1] = card(DECREE, 1, Location::SpellTrapZone, 0);
        with(&obs, &d, |t| assert_eq!(activate(t, &rain), Some(false)));
    }

    #[test]
    fn exact_modes_and_sources_leave_unmodeled_effects_untouched() {
        let obs = observation(0);
        let original = activation(POISON, 0, Location::Hand);
        for invalid in 0..4 {
            let mut choice = original.clone();
            match invalid {
                0 => choice.description = (POISON as u64) << 20,
                1 => choice.card.as_mut().unwrap().at.location = Location::Graveyard,
                2 => choice.card.as_mut().unwrap().at.controller = 1,
                _ => choice.kind = ChoiceKind::SetSpellTrap,
            }
            let d = decision(vec![choice.clone()]);
            with(&obs, &d, |t| assert_eq!(activate(t, &choice), None));
        }
        let choice = activation(SOUL, 0, Location::Hand);
        let d = decision(vec![choice.clone()]);
        with(&obs, &d, |t| assert_eq!(activate(t, &choice), None));
    }

    #[test]
    fn poison_option_uses_native_descriptions_and_safe_recipient() {
        let mut obs = observation(0);
        let mut d = decision(vec![]);
        d.kind = DecisionKind::Option;
        d.choices = [0, 1]
            .map(|slot| Choice {
                kind: ChoiceKind::Option,
                card: None,
                members: vec![],
                description: ((POISON as u64) << 20) + slot,
                place: None,
            })
            .to_vec();
        with(&obs, &d, |t| {
            t.memory.last_activated = Some(POISON);
            assert_eq!(option(t), Some(1));
        });
        obs.cards
            .push(card(BLACK_WINGED, 1, Location::MonsterZone, 0));
        with(&obs, &d, |t| {
            t.memory.last_activated = Some(POISON);
            assert_eq!(option(t), Some(0));
        });
        obs.cards.push(card(NURSE, 1, Location::MonsterZone, 1));
        let choice = activation(POISON, 0, Location::Hand);
        with(&obs, &decision(vec![choice.clone()]), |t| {
            assert_eq!(activate(t, &choice), Some(false))
        });
        d.choices[0].description = (POISON as u64) * 16;
        d.choices[1].description = (POISON as u64) * 16 + 1;
        with(&obs, &d, |t| {
            t.memory.last_activated = Some(POISON);
            assert_eq!(option(t), None);
        });
    }

    #[test]
    fn count_formulas_use_only_visible_resources_and_chain_position() {
        let mut obs = observation(0);
        obs.cards.push(card(0, 1, Location::MonsterZone, 0));
        obs.cards.push(card(0, 1, Location::MonsterZone, 1));
        obs.cards.push(card(OOKAZI, 0, Location::Graveyard, 0));
        obs.cards.push(card(RAIN, 0, Location::Graveyard, 1));
        obs.cards.push(card(NURSE, 0, Location::Graveyard, 2));
        obs.chain = vec![link(OOKAZI, 1, 0), link(RAIN, 0, 0)];
        with(&obs, &decision(vec![]), |t| {
            assert_eq!(
                forecast(t, DESSERTS, false, Some(Location::SpellTrapZone))
                    .unwrap()
                    .loss[1],
                1000
            );
            assert_eq!(
                forecast(t, EXPLOSION, false, Some(Location::SpellTrapZone))
                    .unwrap()
                    .loss[1],
                400
            );
            assert_eq!(
                forecast(t, STRIKE, false, Some(Location::SpellTrapZone))
                    .unwrap()
                    .loss[1],
                1200
            );
        });
        obs.chain_known = false;
        with(&obs, &decision(vec![]), |t| {
            assert!(forecast(t, STRIKE, false, Some(Location::SpellTrapZone)).is_none())
        });
    }

    #[test]
    fn upstart_keeps_draw_value_but_never_claims_recovery_when_deck_empty() {
        let mut obs = observation(0);
        let choice = activation(UPSTART, 0, Location::Hand);
        let d = decision(vec![choice.clone()]);
        with(&obs, &d, |t| assert_eq!(activate(t, &choice), Some(true)));
        obs.pile_sizes[0].2 = 0;
        obs.life_points[1] = 1000;
        obs.cards.push(card(NURSE, 0, Location::MonsterZone, 0));
        with(&obs, &d, |t| {
            assert_eq!(activate(t, &choice), Some(false));
            assert_eq!(activate_before_development(t), None);
        });
    }

    #[test]
    fn monster_continuous_modifier_remains_during_its_activated_effect() {
        let choice = activation(BARREL, 0, Location::SpellTrapZone);
        let d = decision(vec![choice.clone()]);
        for code in [PRIME_MATERIAL, BLACK_WINGED, DRACO_EQUISTE] {
            let mut obs = observation(0);
            obs.life_points[1] = 200;
            let source = card(code, 1, Location::MonsterZone, 0);
            obs.chain.push(ChainLink {
                code,
                controller: 1,
                source: source.at,
                targets: vec![],
            });
            obs.cards.push(source);
            with(&obs, &d, |t| {
                assert_eq!(
                    chain(t, &choice).map(|response| response.score > 0.0),
                    Some(false),
                    "pending monster {code}"
                );
                assert_eq!(activate_before_development(t), None);
            });
        }
    }

    #[test]
    fn pending_drain_and_decree_do_not_disable_existing_damage_protection() {
        let choice = activation(BARREL, 0, Location::SpellTrapZone);
        let d = decision(vec![choice.clone()]);
        let mut obs = observation(0);
        obs.life_points[1] = 200;
        obs.cards
            .push(card(BLACK_WINGED, 1, Location::MonsterZone, 0));
        obs.cards.push(card(
            crate::knowledge::SKILL_DRAIN,
            0,
            Location::SpellTrapZone,
            0,
        ));
        obs.chain.push(link(crate::knowledge::SKILL_DRAIN, 0, 0));
        with(&obs, &d, |t| {
            assert_eq!(
                chain(t, &choice).map(|response| response.score > 0.0),
                Some(false)
            )
        });
        obs.chain.clear();
        with(&obs, &d, |t| {
            assert_eq!(
                chain(t, &choice).map(|response| response.score > 0.0),
                Some(true)
            )
        });
        obs.cards.push(card(DECREE, 0, Location::SpellTrapZone, 1));
        let spell = activation(OOKAZI, 0, Location::Hand);
        with(&obs, &decision(vec![spell.clone()]), |t| {
            assert_eq!(
                chain(t, &spell).map(|response| response.score > 0.0),
                Some(false)
            )
        });
        obs.cards[0] = card(NURSE, 0, Location::MonsterZone, 0);
        let rain = activation(RAIN, 0, Location::Hand);
        with(&obs, &decision(vec![rain.clone()]), |t| {
            assert_eq!(activate(t, &rain), Some(true))
        });
    }

    #[test]
    fn public_conditional_negator_uses_hand_size_after_activation() {
        let mut obs = observation(0);
        obs.life_points[1] = 800;
        obs.cards.push(card(
            crate::knowledge::FABLED_UNICORE,
            1,
            Location::MonsterZone,
            0,
        ));
        obs.pile_sizes
            .extend([(0, Location::Hand, 2), (1, Location::Hand, 1)]);
        let spell = activation(OOKAZI, 0, Location::Hand);
        with(&obs, &decision(vec![spell.clone()]), |t| {
            assert_eq!(activate(t, &spell), Some(false));
            assert_eq!(activate_before_development(t), None);
        });
        let set = activation(OOKAZI, 0, Location::SpellTrapZone);
        with(&obs, &decision(vec![set.clone()]), |t| {
            assert_eq!(activate(t, &set), Some(true))
        });
        obs.cards.push(card(
            crate::knowledge::SKILL_DRAIN,
            0,
            Location::SpellTrapZone,
            0,
        ));
        obs.chain.push(link(crate::knowledge::SKILL_DRAIN, 0, 0));
        with(&obs, &decision(vec![spell.clone()]), |t| {
            assert_eq!(
                chain(t, &spell).map(|response| response.score > 0.0),
                Some(false)
            )
        });
    }

    #[test]
    fn same_code_hand_and_set_choices_project_only_the_selected_copy() {
        for me in [0, 1] {
            let opp = 1 - me;
            for (our_hand, hand_ok, set_ok) in [(1, true, false), (2, false, true)] {
                let mut obs = observation(me);
                obs.life_points[opp as usize] = 800;
                obs.cards.push(card(
                    crate::knowledge::FABLED_UNICORE,
                    opp,
                    Location::MonsterZone,
                    0,
                ));
                obs.pile_sizes
                    .extend([(me, Location::Hand, our_hand), (opp, Location::Hand, 1)]);
                let hand = activation(POISON, me, Location::Hand);
                let set = activation(POISON, me, Location::SpellTrapZone);
                with(&obs, &decision(vec![hand.clone(), set.clone()]), |t| {
                    assert_eq!(activate(t, &hand), Some(hand_ok));
                    assert_eq!(activate(t, &set), Some(set_ok));
                    assert_eq!(activate_before_development(t), Some(usize::from(!hand_ok)));
                });
            }
        }
    }

    #[test]
    fn ordinary_damage_leaves_priority_for_negation_but_lethal_crosses_own_top() {
        let mut obs = observation(0);
        obs.pile_sizes.push((1, Location::Hand, 4));
        obs.chain.push(link(12580477, 1, 0));
        let burn = activation(BARREL, 0, Location::SpellTrapZone);
        let d = decision(vec![burn.clone()]);
        with(&obs, &d, |t| {
            let score = chain(t, &burn).unwrap().score;
            assert_eq!(score, 35.0);
            assert!(score < 80.0); // Existing generic counter/negation priority.
        });
        obs.chain = vec![link(RAIN, 0, 0)];
        with(&obs, &d, |t| {
            assert_eq!(chain(t, &burn).unwrap().score, 0.0)
        });
        obs.life_points[1] = 800;
        with(&obs, &d, |t| {
            assert_eq!(chain(t, &burn).unwrap().score, 150.0)
        });
        let setup = activation(SIMOCHI, 0, Location::SpellTrapZone);
        with(&obs, &decision(vec![setup.clone()]), |t| {
            assert_eq!(chain(t, &setup).unwrap().score, 110.0)
        });
    }
}
