//! Tactical facts for observed individual cards, independent of deck identity.
use crate::agent::{Outcome, Response, Turn};
use crate::cards::{attributes, races};
use crate::ctx::{Ctx, Meets};
use crate::knowledge::{AtkChange, Burn, Fate};
use crate::model::{CardView, Choice, ChoiceKind, DecisionKind, Hint, Location, Phase};
use crate::tactics;

const HONEST: u32 = 37742478;
// vendor/CardScripts/utility.lua Auxiliary.Stringid: (code << 20) | slot.
const RETURN_TO_HAND: u64 = (HONEST as u64) << 20;
const HAND_BOOST: u64 = RETURN_TO_HAND + 1;
const COMMANDER: u32 = 21454943;
const COMMANDER_EFFECT: u64 = (COMMANDER as u64) << 20;
const COMMANDER_PAYMENT: u64 = COMMANDER_EFFECT | 1;

pub const SUPPORTED_CODES: &[u32] = &[HONEST, COMMANDER];

#[derive(Clone, Copy)]
enum BoostAmount {
    OpposingAttack,
}

struct BattleBoostFact {
    code: u32,
    source: Location,
    description: u64,
    eligible_attribute: u32,
    amount: BoostAmount,
}

const HONEST_BOOST: BattleBoostFact = BattleBoostFact {
    code: HONEST,
    source: Location::Hand,
    description: HAND_BOOST,
    eligible_attribute: attributes::LIGHT,
    amount: BoostAmount::OpposingAttack,
};

impl BattleBoostFact {
    fn amount(&self, opposing: &CardView) -> i32 {
        match self.amount {
            BoostAmount::OpposingAttack => opposing.attack.max(0),
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Return,
    Boost,
    Invalid,
}

fn mode(t: &Turn, choice: &Choice) -> Option<Mode> {
    if t.ctx.canonical(choice.code()?) != HONEST {
        return None;
    }
    Some(match choice.at() {
        Some(at) if choice.kind == ChoiceKind::Activate && at.controller == t.ctx.me => {
            match (at.location, choice.description) {
                (source, description)
                    if source == HONEST_BOOST.source && description == HONEST_BOOST.description =>
                {
                    Mode::Boost
                }
                (Location::MonsterZone, RETURN_TO_HAND) => Mode::Return,
                _ => Mode::Invalid,
            }
        }
        _ => Mode::Invalid,
    })
}

fn light_combatant(ctx: &Ctx, card: &CardView) -> bool {
    card.at.controller == ctx.me
        && card.at.location == Location::MonsterZone
        && card.position.face_up
        && card.position.attack
        && ctx.view_data(card).attribute & HONEST_BOOST.eligible_attribute != 0
}

fn available_trick(ctx: &Ctx, attacker: &CardView) -> bool {
    light_combatant(ctx, attacker)
        && ctx.in_hand(HONEST_BOOST.code)
        // c37742478 uses Cost.SelfToGrave: a banishment replacement prevents
        // paying the cost even though Honest is visibly available in hand.
        && !ctx.monsters_banished()
        && !ctx.wasted(HONEST_BOOST.code)
}

/// This target-independent API cannot use the strongest opponent's ATK for
/// every target. Its bound is zero for any hidden target; exact target handling
/// below uses that particular opponent's current ATK, never its DEF.
pub(super) fn attack_trick(ctx: &Ctx, attacker: &CardView) -> i32 {
    if !available_trick(ctx, attacker) {
        return 0;
    }
    ctx.monsters(ctx.opp)
        .iter()
        .map(|card| {
            if card.position.face_up {
                HONEST_BOOST.amount(card)
            } else {
                0
            }
        })
        .min()
        .unwrap_or(0)
}

pub(super) fn attack_outcome(ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
    if !available_trick(ctx, attacker) {
        return None;
    }
    let before = tactics::default_outcome(ctx, attacker, target, 0);
    if target.at.controller != ctx.opp || !target.position.face_up || target.attack <= 0 {
        return Some(before);
    }
    let mut boosted = attacker.clone();
    boosted.attack = boosted.attack.saturating_add(HONEST_BOOST.amount(target));
    let after = tactics::default_outcome(ctx, &boosted, target, 0);
    let damage = damage(ctx, &boosted, target);
    if damage[ctx.me as usize] >= ctx.my_lp() {
        return Some(before);
    }
    Some(
        if !matches!(before, Outcome::Win { .. }) && matches!(after, Outcome::Win { .. }) {
            Outcome::Win { trick: true }
        } else {
            before
        },
    )
}

/// Public battle damage, including existing shared facts about piercing,
/// reflected damage and damage-step removal. Used only to justify spending
/// Honest for lethal or to avoid an otherwise fatal battle.
fn damage(ctx: &Ctx, attacker: &CardView, target: &CardView) -> [i32; 2] {
    let met = ctx.attack_meets(Some(attacker), target);
    damage_with_meets(ctx, attacker, target, met)
}

fn damage_with_meets(ctx: &Ctx, attacker: &CardView, target: &CardView, met: Meets) -> [i32; 2] {
    if (met.facts.before_damage && met.facts.attacker != Fate::Unharmed)
        || ctx.strike_removes(attacker, target) == Some(true)
    {
        return [0; 2];
    }
    let strike = ctx.facts(attacker).striking;
    let attack = met
        .facts
        .attacker_atk
        .apply(attacker.attack.saturating_add(strike.bonus));
    let delta = attack.saturating_sub(met.stat);
    let mut received = [0i32; 2];
    if delta < 0 {
        received[attacker.at.controller as usize] = delta.saturating_neg();
    }
    let defender_damage = if delta > 0 && (!met.defending || strike.piercing) {
        delta
    } else {
        0
    };
    if !met.facts.no_damage {
        received[target.at.controller as usize] = defender_damage;
    }
    let burn = match met.facts.burn {
        Burn::None => 0,
        Burn::Fixed(value) => value,
        Burn::AttackerAtk => attack,
        Burn::Reflected => {
            received[target.at.controller as usize] = 0;
            defender_damage
        }
    };
    received[attacker.at.controller as usize] =
        received[attacker.at.controller as usize].saturating_add(burn.max(0));
    received
}

/// Commander pays 100--500 LP to reduce the opposing combatant's ATK and DEF
/// by the chosen amount. The legal offered effect is the timing authority;
/// neither a particular deck nor a guessed future attack supplies context.
fn commander_choice(t: &Turn, choice: &Choice) -> bool {
    matches!(t.decision.kind, DecisionKind::Chain { triggers: false, .. })
        && choice.kind == ChoiceKind::Activate
        && choice.description == COMMANDER_EFFECT
        && choice.code().is_some_and(|code| t.ctx.canonical(code) == COMMANDER)
        && t.decision.choices.contains(choice)
        && choice.at().is_some_and(|at| at.controller == t.ctx.me && at.location == Location::MonsterZone)
        && t.view(choice).is_some_and(|card| card.position.face_up && t.ctx.is(card, COMMANDER))
}

fn commander_result(ctx: &Ctx, attacker: &CardView, target: &CardView) -> (Outcome, [i32; 2]) {
    let mut met = ctx.attack_meets(Some(attacker), target);
    // The shared probe assumes a prospective 100-point Commander activation.
    // This transaction compares actual current numbers and an explicit paid
    // reduction; retaining the probe would count that reduction twice. Keep
    // every other public battle fact and the card's printed identity intact.
    if ctx.is(target, COMMANDER) && met.facts.attacker_atk == AtkChange::Minus(100) {
        met.facts.attacker_atk = AtkChange::None;
    }
    (tactics::default_outcome_with_meets(ctx, attacker, target, 0, met),
        damage_with_meets(ctx, attacker, target, met))
}

fn commander_rank(outcome: Outcome, attacking: bool) -> u8 {
    match (outcome, attacking) {
        (Outcome::Win { .. }, true) | (Outcome::Lose, false) => 3,
        (Outcome::Bounce, _) => 2,
        (Outcome::Trade, _) => 1,
        _ => 0,
    }
}

/// Outer None means the public transaction cannot be identified/evaluated.
/// Some(None) is an identified, pointless or unsafe payment. Prefer the best
/// safe battle result, then its cheapest offered amount, rather than paying
/// less for a mutual destruction when a slightly larger amount saves ours.
fn commander_payment(ctx: &Ctx, amounts: impl IntoIterator<Item = i32>, paying: bool) -> Option<Option<i32>> {
    let (attacker, target) = (ctx.battle_attacker()?, ctx.battle_target()?);
    // OCGCore AddChain step2 publishes CHAINING before step6 runs the LP
    // cost. At the announcement, the sole link must identify this transaction.
    // The hint alone can survive a previous no-announcement 100-LP cost.
    let chain_identified = if paying {
        matches!(ctx.obs.chain.as_slice(), [link] if link.controller == ctx.me
            && ctx.canonical(link.code) == COMMANDER && link.source.controller == ctx.me
            && link.source.location == Location::MonsterZone && link.targets.is_empty()
            && ctx.card(link.source).is_some_and(|card| card.position.face_up && ctx.is(card, COMMANDER)))
    } else { ctx.obs.chain.is_empty() };
    if !ctx.obs.chain_known || !chain_identified
        || !ctx.phase().is_some_and(|phase| phase.is_battle() && phase != Phase::DamageCalculation)
        || [attacker, target].into_iter().any(|card| !card.known() || !card.position.face_up
            || card.at.location != Location::MonsterZone)
        || !attacker.position.attack || attacker.at.controller == target.at.controller
    { return None; }
    let attacking = attacker.at.controller == ctx.me;
    let (ours, theirs) = if attacking { (attacker, target) } else { (target, attacker) };
    if ours.at.controller != ctx.me || theirs.at.controller != ctx.opp
        || ctx.view_data(ours).race & races::PSYCHIC == 0
        || !ctx.monsters(ctx.me).into_iter().any(|card| card.position.face_up && ctx.is(card, COMMANDER))
    { return None; }
    if ctx.effects_drained() || ctx.wasted(COMMANDER) || !ctx.reaches(theirs, COMMANDER, true, false) {
        return Some(None);
    }
    // These public shield facts require a fuller damage-recipient calculation
    // than the shared combat helper offers. Do not invent lethal or LP savings.
    if ctx.obs.cards.iter().any(|card| card.at.location.is_field() && card.position.face_up
        && card.code.is_some_and(|code| super::knowledge::profile(ctx.canonical(code)).damage_shield.is_some()))
    { return None; }
    let (before, before_damage) = commander_result(ctx, attacker, target);
    let before_rank = commander_rank(before, attacking);
    let me = ctx.me as usize;
    let opp = ctx.opp as usize;
    let mut best: Option<((bool, bool, u8), i32)> = None;
    for amount in amounts {
        if !(100..=500).contains(&amount) || amount % 100 != 0
            || amount >= ctx.my_lp() || amount > theirs.attack.max(theirs.defense)
        { continue; }
        let mut reduced = theirs.clone();
        reduced.attack = reduced.attack.saturating_sub(amount).max(0);
        reduced.defense = reduced.defense.saturating_sub(amount).max(0);
        let (after_attacker, after_target) = if attacking { (attacker, &reduced) } else { (&reduced, target) };
        let (after, after_damage) = commander_result(ctx, after_attacker, after_target);
        // Paying LP is distinct from battle damage. A 300-point reduction
        // which saves 300 damage buys no LP survival if the body still loses.
        if amount.saturating_add(after_damage[me]) >= ctx.my_lp() { continue; }
        let lethal = before_damage[opp] < ctx.opp_lp() && after_damage[opp] >= ctx.opp_lp();
        let saves_life = before_damage[me] >= ctx.my_lp();
        let rank = commander_rank(after, attacking);
        if !lethal && !saves_life && rank <= before_rank { continue; }
        let quality = (lethal, saves_life, rank);
        if best.is_none_or(|(old, paid)| quality > old || quality == old && amount < paid) {
            best = Some((quality, amount));
        }
    }
    Some(best.map(|(_, amount)| amount))
}

fn commander_score(t: &Turn, choice: &Choice) -> Option<f64> {
    if !commander_choice(t, choice) { return None; }
    commander_payment(&t.ctx, [100, 200, 300, 400, 500], false)
        .map(|payment| if payment.is_some() { 95.0 } else { 0.0 })
}

pub(super) fn announce(t: &Turn) -> Option<usize> {
    if t.decision.kind != DecisionKind::Announce
        || t.decision.hint != Hint::Other(COMMANDER_PAYMENT)
        || t.memory.last_activated.map(|code| t.ctx.canonical(code)) != Some(COMMANDER)
        || t.decision.choices.iter().any(|choice| choice.kind != ChoiceKind::Announce
            || choice.card.is_some() || !choice.members.is_empty()
            || !(100..=500).contains(&choice.description) || choice.description % 100 != 0)
    { return None; }
    let amount = commander_payment(&t.ctx, t.decision.choices.iter().map(|choice| choice.description as i32), true)??;
    t.choices().find(|(_, choice)| choice.description == amount as u64).map(|(index, _)| index)
}

fn boost_score(t: &Turn) -> f64 {
    let ctx = t.ctx;
    if ctx.wasted(HONEST)
        || ctx.monsters_banished()
        || ctx.obs.chain.iter().any(|link| {
            ctx.canonical(link.code) == HONEST && link.source.location == Location::Hand
        })
    {
        return 0.0;
    }
    let (Some(attacker), Some(target)) = (ctx.battle_attacker(), ctx.battle_target()) else {
        return 0.0;
    };
    if !attacker.position.face_up || !target.position.face_up {
        return 0.0;
    }
    let attacking = attacker.at.controller == ctx.me;
    let (ours, theirs) = if attacking {
        (attacker, target)
    } else {
        (target, attacker)
    };
    if !light_combatant(&ctx, ours) || theirs.at.controller != ctx.opp || theirs.attack <= 0 {
        return 0.0;
    }
    // c37742478 permits only its legal Damage Step window, but the adapter's
    // public phase may still say BattleStart. The offered exact hand effect is
    // the timing authority; a Defense Position defender gains no useful DEF.
    let before = tactics::default_outcome(&ctx, attacker, target, 0);
    let before_damage = damage(&ctx, attacker, target);
    let mut boosted = ours.clone();
    boosted.attack = boosted.attack.saturating_add(HONEST_BOOST.amount(theirs));
    let (after_attacker, after_target) = if attacking {
        (&boosted, target)
    } else {
        (attacker, &boosted)
    };
    let after = tactics::default_outcome(&ctx, after_attacker, after_target, 0);
    let after_damage = damage(&ctx, after_attacker, after_target);
    if after_damage[ctx.me as usize] >= ctx.my_lp() {
        return 0.0;
    }
    if before_damage[ctx.opp as usize] < ctx.opp_lp()
        && after_damage[ctx.opp as usize] >= ctx.opp_lp()
    {
        return 150.0;
    }
    if before_damage[ctx.me as usize] >= ctx.my_lp() {
        return 140.0;
    }
    let useful = if attacking {
        (!matches!(before, Outcome::Win { .. }) && matches!(after, Outcome::Win { .. }))
            || (before == Outcome::Lose && after == Outcome::Bounce)
    } else {
        matches!(before, Outcome::Win { .. } | Outcome::Trade)
            && matches!(after, Outcome::Lose | Outcome::Bounce)
    };
    if useful {
        95.0
    } else {
        0.0
    }
}

fn return_useful(t: &Turn, choice: &Choice) -> bool {
    let ctx = t.ctx;
    let Some(source) = t.view(choice) else {
        return false;
    };
    // Retain the body through its possible attack and never empty our board.
    // In Main2 another LIGHT attacker can instead use Honest defensively next
    // turn. An existing hand copy makes sacrificing this body unnecessary.
    ctx.my_turn()
        && ctx.phase() == Some(Phase::Main2)
        && source.position.face_up
        && !ctx.effects_drained()
        && !ctx.wasted(HONEST)
        && ctx.obs.chain.is_empty()
        && !ctx.in_hand(HONEST)
        && ctx
            .monsters(ctx.me)
            .iter()
            .any(|card| card.at != source.at && light_combatant(&ctx, card) && card.attack > 0)
}

pub(super) fn allow_activation(t: &Turn, choice: &Choice) -> Option<bool> {
    if let Some(score) = commander_score(t, choice) { return Some(score > 0.0); }
    Some(match mode(t, choice)? {
        Mode::Return => return_useful(t, choice),
        Mode::Boost => boost_score(t) > 0.0,
        Mode::Invalid => false,
    })
}

pub(super) fn chain(t: &Turn, index: usize) -> Option<Response> {
    let choice = t.choice(index);
    if let Some(score) = commander_score(t, choice) { return Some(Response::new(score)); }
    Some(match mode(t, choice)? {
        Mode::Boost => Response::new(boost_score(t)),
        Mode::Return if return_useful(t, choice) => Response::new(30.0),
        _ => Response::no(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, Memory, Strategy};
    use crate::cards::{types, CardData, MemoryCards};
    use crate::model::{
        CardRef, ChainLink, Decision, DecisionKind, Hint, Member, Observation, Position,
    };
    use crate::Policy;
    use std::sync::Arc;

    const LIGHT: u32 = 900_700_001;
    const DARK: u32 = 900_700_002;
    fn database() -> MemoryCards {
        MemoryCards(
            [
                (HONEST, 1100, 1900, attributes::LIGHT),
                (LIGHT, 1500, 1000, attributes::LIGHT),
                (DARK, 2000, 2000, attributes::DARK),
            ]
            .into_iter()
            .map(|(code, attack, defense, attribute)| {
                (
                    code,
                    CardData {
                        code,
                        kind: types::MONSTER | types::EFFECT,
                        attack,
                        defense,
                        attribute,
                        level: 4,
                        ..CardData::default()
                    },
                )
            })
            .collect(),
        )
    }
    fn card(code: u32, controller: u8, location: Location, sequence: u32, attack: i32) -> CardView {
        CardView {
            at: CardRef {
                controller,
                location,
                sequence,
            },
            code: Some(code),
            position: Position::FACE_UP_ATTACK,
            attack,
            defense: 2000,
            level: 4,
            can_attack: true,
            battles: 0,
            counters: 0,
            coin_effect: None, negated: false,
        }
    }
    fn observation(attacking: bool) -> Observation {
        let ours = card(LIGHT, 0, Location::MonsterZone, 0, 1500);
        let theirs = card(DARK, 1, Location::MonsterZone, 0, 2000);
        Observation {
            me: 0,
            turn: 2,
            turn_player: Some(if attacking { 0 } else { 1 }),
            phase: Some(Phase::BattleStart),
            life_points: [8000; 2],
            battle_attacker: Some(if attacking { ours.at } else { theirs.at }),
            battle_target: Some(if attacking { theirs.at } else { ours.at }),
            cards: vec![ours, theirs, card(HONEST, 0, Location::Hand, 0, 1100)],
            pile_sizes: vec![],
            chain: vec![],
            event_cards: vec![],
            summon_used: false,
            chain_known: true,
            can_attack_known: true,
            coin_toss: None,
        }
    }
    fn activation(location: Location, description: u64) -> Choice {
        Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member {
                at: CardRef {
                    controller: 0,
                    location,
                    sequence: 0,
                },
                code: Some(HONEST),
                value: 0,
                required: false,
            }),
            members: vec![],
            description,
            place: None,
        }
    }
    fn decision(kind: DecisionKind, choice: Choice) -> Decision {
        Decision {
            kind,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: None,
            choices: vec![
                choice,
                Choice {
                    kind: ChoiceKind::Pass,
                    card: None,
                    members: vec![],
                    description: 0,
                    place: None,
                },
            ],
        }
    }
    fn score(obs: &Observation, choice: Choice) -> f64 {
        let db = database();
        let mut memory = Memory::default();
        let prompt = decision(
            DecisionKind::Chain {
                forced: false,
                triggers: false,
            },
            choice,
        );
        chain(
            &Turn {
                ctx: Ctx::new(obs, &db),
                decision: &prompt,
                memory: &mut memory,
            },
            0,
        )
        .unwrap()
        .score
    }

    #[test]
    fn hand_boost_wins_and_saves_battles_on_both_turns_even_with_stale_phase() {
        // Native effect description, independent of the implementation constant:
        // observed return slot 39575856611328 plus Lua's hand-effect slot 1.
        let native_hand_description = 39_575_856_611_329;
        for attacking in [false, true] {
            let obs = observation(attacking);
            assert!(score(&obs, activation(Location::Hand, native_hand_description)) > 0.0);
            let mut agent = Agent::new(crate::generic::Generic, Arc::new(database()));
            assert_eq!(
                agent.choose(
                    &obs,
                    &decision(
                        DecisionKind::Chain {
                            forced: false,
                            triggers: false
                        },
                        activation(Location::Hand, native_hand_description)
                    )
                ),
                0
            );
        }
    }

    #[test]
    fn exact_effect_and_range_reject_wrong_modes_and_useless_boosts() {
        let mut obs = observation(true);
        assert_eq!(
            score(&obs, activation(Location::Hand, HONEST as u64 * 16 + 1)),
            0.0
        );
        assert_eq!(score(&obs, activation(Location::Hand, RETURN_TO_HAND)), 0.0);
        assert_eq!(
            score(&obs, activation(Location::Graveyard, HAND_BOOST)),
            0.0
        );
        obs.cards[0].attack = 3000;
        assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
        obs.cards[0].code = Some(DARK);
        obs.cards[0].attack = 1000;
        assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
        let mut defending = observation(false);
        defending.cards[0].position = Position::FACE_UP_DEFENSE;
        assert_eq!(
            score(&defending, activation(Location::Hand, HAND_BOOST)),
            0.0
        );
        obs = observation(true);
        obs.cards[1].attack = 100;
        obs.cards[1].defense = 3000;
        obs.cards[1].position = Position::FACE_UP_DEFENSE;
        assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
    }

    #[test]
    fn exact_target_attack_plan_handles_ties_without_borrowing_other_targets_attack() {
        let db = database();
        let mut obs = observation(true);
        obs.cards[0].attack = 2000;
        let ctx = Ctx::new(&obs, &db);
        assert_eq!(
            attack_outcome(&ctx, &obs.cards[0], &obs.cards[1]),
            Some(Outcome::Win { trick: true })
        );
        obs.cards[1].attack = 100;
        obs.cards[1].defense = 3000;
        obs.cards[1].position = Position::FACE_UP_DEFENSE;
        obs.cards
            .push(card(DARK, 1, Location::MonsterZone, 1, 4000));
        let ctx = Ctx::new(&obs, &db);
        assert_eq!(attack_trick(&ctx, &obs.cards[0]), 100);
        assert_eq!(
            attack_outcome(&ctx, &obs.cards[0], &obs.cards[1]),
            Some(Outcome::Bounce)
        );
        obs.cards[1].position = Position::FACE_DOWN_DEFENSE;
        obs.cards[1].code = None;
        let ctx = Ctx::new(&obs, &db);
        assert_eq!(attack_trick(&ctx, &obs.cards[0]), 0);
    }

    #[test]
    fn boost_spends_for_lethal_but_waits_for_unresolved_honest() {
        let mut obs = observation(true);
        obs.cards[0].attack = 3000;
        obs.life_points[1] = 2000;
        assert!(score(&obs, activation(Location::Hand, HAND_BOOST)) > 100.0);
        for controller in [0, 1] {
            obs.chain = vec![ChainLink {
                code: HONEST,
                controller,
                source: CardRef {
                    controller,
                    location: Location::Hand,
                    sequence: 1,
                },
                targets: vec![],
            }];
            assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
        }
    }

    #[test]
    fn reflected_damage_cannot_turn_an_honest_boost_into_a_safe_win() {
        let mut obs = observation(true);
        // Existing shared combat knowledge: Amazoness Swords Woman reflects
        // her controller's battle damage even though no_damage is also true.
        obs.cards[1].code = Some(94004268);
        obs.life_points[0] = 1000;
        let db = database();
        let ctx = Ctx::new(&obs, &db);
        let mut boosted = obs.cards[0].clone();
        boosted.attack += obs.cards[1].attack;
        assert_eq!(damage(&ctx, &boosted, &obs.cards[1]), [1500, 0]);
        assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
        assert_eq!(
            attack_outcome(&ctx, &obs.cards[0], &obs.cards[1]),
            Some(Outcome::Lose)
        );
    }

    #[test]
    fn banishment_replacement_prevents_planning_an_unpayable_hand_boost() {
        let db = database();
        for (code, location) in [
            (30241314, Location::SpellTrapZone),
            (81674782, Location::SpellTrapZone),
            (94853057, Location::MonsterZone),
        ] {
            let mut obs = observation(true);
            obs.cards.push(card(code, 1, location, 1, 1600));
            let ctx = Ctx::new(&obs, &db);
            assert!(ctx.monsters_banished());
            assert_eq!(attack_trick(&ctx, &obs.cards[0]), 0);
            assert_eq!(attack_outcome(&ctx, &obs.cards[0], &obs.cards[1]), None);
            assert_eq!(score(&obs, activation(Location::Hand, HAND_BOOST)), 0.0);
        }
    }

    #[test]
    fn field_return_keeps_attack_and_sole_body_but_can_recover_main2_trick() {
        let mut obs = observation(true);
        obs.phase = Some(Phase::Main1);
        obs.cards.truncate(2);
        obs.cards[0].code = Some(HONEST);
        obs.cards[0].attack = 1100;
        let effect = activation(Location::MonsterZone, 39_575_856_611_328);
        for kind in [
            DecisionKind::Idle,
            DecisionKind::Chain {
                forced: false,
                triggers: false,
            },
        ] {
            let mut prompt = decision(kind, effect.clone());
            prompt.choices[1].kind = if kind == DecisionKind::Idle {
                ChoiceKind::EndTurn
            } else {
                ChoiceKind::Pass
            };
            let mut agent = Agent::new(crate::generic::Generic, Arc::new(database()));
            assert_ne!(agent.choose(&obs, &prompt), 0);
        }
        obs.phase = Some(Phase::Main2);
        assert_eq!(score(&obs, effect.clone()), 0.0);
        obs.cards
            .push(card(LIGHT, 0, Location::MonsterZone, 1, 1800));
        assert!(score(&obs, effect.clone()) > 0.0);
        let db = database();
        let mut memory = Memory::default();
        let prompt = decision(DecisionKind::Idle, effect.clone());
        assert_eq!(
            allow_activation(
                &Turn {
                    ctx: Ctx::new(&obs, &db),
                    decision: &prompt,
                    memory: &mut memory
                },
                &effect
            ),
            Some(true)
        );
    }

    #[test]
    fn hidden_opponent_hand_identity_does_not_change_tactical_decisions() {
        let db = database();
        let mut obs = observation(true);
        let expected = score(&obs, activation(Location::Hand, HAND_BOOST));
        let expected_outcome = attack_outcome(&Ctx::new(&obs, &db), &obs.cards[0], &obs.cards[1]);
        let mut hidden = card(DARK, 1, Location::Hand, 0, 9999);
        hidden.code = None;
        obs.cards.push(hidden);
        for code in [None, Some(HONEST), Some(DARK)] {
            obs.cards.last_mut().unwrap().code = code;
            assert_eq!(
                score(&obs, activation(Location::Hand, HAND_BOOST)),
                expected
            );
            assert_eq!(
                attack_outcome(&Ctx::new(&obs, &db), &obs.cards[0], &obs.cards[1]),
                expected_outcome
            );
        }
        // The generic forwards remain target-specific; they do not dispatch
        // by a recognized deck or examine the opponent's unrevealed cards.
        assert_eq!(
            crate::generic::Generic.attack_outcome(
                &Ctx::new(&obs, &db),
                &obs.cards[0],
                &obs.cards[1]
            ),
            expected_outcome
        );
    }

    fn commander_database() -> MemoryCards {
        let mut db = database();
        db.0.insert(COMMANDER, CardData { code: COMMANDER, kind: types::MONSTER | types::EFFECT,
            attack: 1400, defense: 800, level: 3, race: races::PSYCHIC,
            attribute: attributes::EARTH, ..CardData::default() });
        db.0.get_mut(&LIGHT).unwrap().race = races::PSYCHIC;
        db
    }

    fn commander_observation(me: u8, attacking: bool) -> Observation {
        let mut obs = observation(attacking);
        obs.cards.truncate(2);
        obs.cards[0].code = Some(COMMANDER);
        obs.cards[0].attack = 1400;
        obs.cards[0].defense = 800;
        obs.cards[1].attack = 1600;
        obs.cards[1].defense = 1200;
        if me == 1 {
            for card in &mut obs.cards { card.at.controller = 1 - card.at.controller; }
            obs.battle_attacker.as_mut().unwrap().controller ^= 1;
            obs.battle_target.as_mut().unwrap().controller ^= 1;
            obs.turn_player = obs.turn_player.map(|player| 1 - player);
        }
        obs.me = me;
        obs
    }

    fn commander_activation(obs: &Observation) -> Choice {
        let source = obs.cards.iter().find(|card| card.code == Some(COMMANDER)
            && card.at.controller == obs.me).unwrap();
        Choice { kind: ChoiceKind::Activate, description: (21_454_943u64) << 20,
            card: Some(Member { at: source.at, code: source.code, value: 0, required: false }),
            members: vec![], place: None }
    }

    fn commander_link(obs: &mut Observation) {
        obs.chain = vec![ChainLink { code: COMMANDER, controller: obs.me,
            source: commander_activation(obs).at().unwrap(), targets: vec![] }];
    }

    fn payment_prompt(values: &[u64]) -> Decision {
        Decision { kind: DecisionKind::Announce, hint: Hint::Other((21_454_943u64 << 20) | 1),
            minimum: 0, maximum: 0, selected: vec![], subject: None,
            choices: values.iter().map(|value| Choice { kind: ChoiceKind::Announce,
                description: *value, card: None, members: vec![], place: None }).collect() }
    }

    fn payment(obs: &Observation, values: &[u64]) -> Option<usize> {
        let db = commander_database();
        let mut memory = Memory::default();
        memory.last_activated = Some(COMMANDER);
        let decision = payment_prompt(values);
        announce(&Turn { ctx: Ctx::new(obs, &db), decision: &decision, memory: &mut memory })
    }

    #[test]
    fn commander_agent_pays_for_a_real_win_on_both_battle_sides_and_seats() {
        for me in [0, 1] {
            for attacking in [false, true] {
                let mut obs = commander_observation(me, attacking);
                let mut agent = Agent::new(crate::generic::Generic, Arc::new(commander_database()));
                let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, commander_activation(&obs));
                assert_eq!(agent.choose(&obs, &prompt), 0);
                // The native engine publishes CHAINING before executing the
                // cost. Its numeric menu order need not be sorted by amount.
                commander_link(&mut obs);
                let prompt = payment_prompt(&[500, 100, 300, 200, 400]);
                assert_eq!(agent.choose(&obs, &prompt), 2,
                    "1400 versus 1600 needs 300 for a win, not the default 100 or a 200-point trade");
            }
        }
    }

    #[test]
    fn commander_uses_the_actual_defense_and_preserves_our_defending_body() {
        for me in [0, 1] {
            let mut attacking = commander_observation(me, true);
            attacking.cards[1].position = Position::FACE_UP_DEFENSE;
            attacking.cards[1].attack = 100;
            attacking.cards[1].defense = 1800;
            commander_link(&mut attacking);
            assert_eq!(payment(&attacking, &[100, 200, 300, 400, 500]), Some(4),
                "400 only ties DEF; 500 destroys the defender");
            let mut defending = commander_observation(me, false);
            defending.cards[0].position = Position::FACE_UP_DEFENSE;
            defending.cards[0].defense = 1400;
            commander_link(&mut defending);
            assert_eq!(payment(&defending, &[500, 300, 200, 100]), Some(2),
                "200 reaches equal DEF and saves ours without paying another 100");
        }
    }

    #[test]
    fn commander_declines_pointless_payments_and_does_not_count_cost_as_damage_prevention() {
        for (our_attack, their_attack, lp) in [(2000, 1600, 8000), (1400, 2000, 550), (1400, 2000, 1000)] {
            let mut obs = commander_observation(0, true);
            obs.cards[0].attack = our_attack;
            obs.cards[1].attack = their_attack;
            obs.life_points[0] = lp;
            let mut agent = Agent::new(crate::generic::Generic, Arc::new(commander_database()));
            let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, commander_activation(&obs));
            assert_eq!(agent.choose(&obs, &prompt), 1, "{our_attack}/{their_attack}/{lp}");
        }
    }

    #[test]
    fn commander_spends_the_smallest_lethal_amount_and_accounts_for_reflected_damage() {
        let mut obs = commander_observation(0, true);
        obs.cards[0].attack = 2000;
        obs.life_points[1] = 600;
        commander_link(&mut obs);
        assert_eq!(payment(&obs, &[500, 300, 200, 100]), Some(2), "400 current damage plus 200 is lethal");
        obs = commander_observation(0, true);
        obs.cards[1].code = Some(94004268); // Existing Swords Woman reflection fact.
        obs.life_points[0] = 400;
        commander_link(&mut obs);
        assert_eq!(payment(&obs, &[100, 200, 300, 400, 500]), Some(1),
            "300 plus 100 reflected damage is fatal; the 200-point trade remains safe");
        obs.life_points[0] = 401;
        assert_eq!(payment(&obs, &[100, 200, 300, 400, 500]), Some(2));
        obs.life_points[0] = 200;
        assert_eq!(payment(&obs, &[100]), None);
    }

    #[test]
    fn commander_respects_public_negation_targeting_immunity_and_nonpsychic_battlers() {
        let db = commander_database();
        for lock in [crate::knowledge::SKILL_DRAIN, crate::knowledge::FABLED_UNICORE] {
            let mut obs = commander_observation(0, true);
            let location = if lock == crate::knowledge::SKILL_DRAIN { Location::SpellTrapZone } else { Location::MonsterZone };
            obs.cards.push(card(lock, 1, location, 1, 2300));
            assert_eq!(commander_payment(&Ctx::new(&obs, &db), [100, 200, 300, 400, 500], false), Some(None));
        }
        let mut obs = commander_observation(0, true);
        obs.cards[1].code = Some(27527047); // Shared monster-effect targeting immunity.
        assert_eq!(commander_payment(&Ctx::new(&obs, &db), [100, 200, 300, 400, 500], false), Some(None));
        obs = commander_observation(0, true);
        obs.cards[0].code = Some(DARK);
        obs.cards.push(card(COMMANDER, 0, Location::MonsterZone, 1, 1400));
        assert_eq!(commander_payment(&Ctx::new(&obs, &db), [100, 200, 300, 400, 500], false), None);
    }

    #[test]
    fn commander_announcement_requires_exact_live_transaction_and_keeps_fallbacks() {
        let mut obs = commander_observation(0, true);
        let db = commander_database();
        let mut memory = Memory::default();
        memory.last_activated = Some(COMMANDER);
        let mut prompt = payment_prompt(&[500, 100, 300, 200]);
        let check = |obs: &Observation, prompt: &Decision, memory: &mut Memory|
            announce(&Turn { ctx: Ctx::new(obs, &db), decision: prompt, memory });
        assert_eq!(check(&obs, &prompt, &mut memory), None, "stale hint without the current chain link");
        commander_link(&mut obs);
        assert_eq!(check(&obs, &prompt, &mut memory), Some(2));
        for hint in [Hint::None, Hint::Other(123), Hint::Other(COMMANDER_EFFECT)] {
            prompt.hint = hint;
            assert_eq!(check(&obs, &prompt, &mut memory), None);
        }
        prompt = payment_prompt(&[500, 100, 300, 200]);
        memory.last_activated = Some(HONEST);
        assert_eq!(check(&obs, &prompt, &mut memory), None);
        memory.last_activated = Some(COMMANDER);
        obs.chain[0].code = HONEST;
        assert_eq!(check(&obs, &prompt, &mut memory), None);
        commander_link(&mut obs);
        obs.chain[0].source.location = Location::Hand;
        assert_eq!(check(&obs, &prompt, &mut memory), None);
        commander_link(&mut obs);
        prompt.choices[0].description = 9;
        assert_eq!(check(&obs, &prompt, &mut memory), None, "unrelated numeric menu");
        prompt = payment_prompt(&[500, 100, 300, 200]);
        obs.battle_target = None;
        assert_eq!(check(&obs, &prompt, &mut memory), None, "missing current battle");
    }

    #[test]
    fn commander_known_rule_accepts_unseen_psychics_without_inspecting_hidden_hands() {
        let db = commander_database();
        let mut obs = commander_observation(0, true);
        obs.cards[0].code = Some(LIGHT); // Synthetic, previously unseen Psychic.
        obs.cards.push(card(COMMANDER, 0, Location::MonsterZone, 1, 1400));
        commander_link(&mut obs);
        assert_eq!(payment(&obs, &[100, 200, 300, 400, 500]), Some(2));
        let mut hidden = card(DARK, 1, Location::Hand, 0, 9999);
        for code in [None, Some(HONEST), Some(COMMANDER)] {
            hidden.code = code;
            obs.cards.push(hidden.clone());
            assert_eq!(payment(&obs, &[100, 200, 300, 400, 500]), Some(2));
            obs.cards.pop();
        }
        let mut memory = Memory::default();
        obs.chain.clear();
        let mut effect = commander_activation(&obs);
        effect.description += 1;
        let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, effect.clone());
        assert_eq!(commander_score(&Turn { ctx: Ctx::new(&obs, &db), decision: &prompt, memory: &mut memory }, &effect), None,
            "an unrelated effect mode must not inherit Commander payment semantics");
    }
}
