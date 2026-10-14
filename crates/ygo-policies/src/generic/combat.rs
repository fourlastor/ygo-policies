//! Tactical facts for observed individual cards, independent of deck identity.
use crate::agent::{Outcome, Response, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::knowledge::{Burn, Fate};
use crate::model::{CardView, Choice, ChoiceKind, Location, Phase};
use crate::tactics;

const HONEST: u32 = 37742478;
// vendor/CardScripts/utility.lua Auxiliary.Stringid: (code << 20) | slot.
const RETURN_TO_HAND: u64 = (HONEST as u64) << 20;
const HAND_BOOST: u64 = RETURN_TO_HAND + 1;

pub const SUPPORTED_CODES: &[u32] = &[HONEST];

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
    Some(match mode(t, choice)? {
        Mode::Return => return_useful(t, choice),
        Mode::Boost => boost_score(t) > 0.0,
        Mode::Invalid => false,
    })
}

pub(super) fn chain(t: &Turn, index: usize) -> Option<Response> {
    let choice = t.choice(index);
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
            coin_effect: None,
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
}
