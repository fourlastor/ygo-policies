//! Public counter support and ordering facts for individual cards.
//!
//! Sources: official/c34029630.lua (bank, fixed one-counter transfer and its
//! optional question), c39910367.lua (collect other Spell activations), and
//! the recipient scripts named below. All identities occur in the active pool.
//! Aggregate `CardView::counters` only ranks prospective thresholds; it is not
//! a typed counter balance or proof that an effect can be paid. Offered native
//! actions and target choices remain the authority on legality.

use super::knowledge;
use crate::agent::Turn;
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{
    CardRef, CardView, Choice, ChoiceKind, DecisionKind, Hint, Location, Member, Phase,
};

const POWER_STONE: u32 = 34029630;
const CITADEL: u32 = 39910367;
const LIBRARY: u32 = 70791313;
const BREAKER: u32 = 71413901;
const BLAST: u32 = 21051146;
const CERBERUS: u32 = 55424270;
const DEFENDER: u32 = 2525268;
const DARK_RED: u32 = 45462639;
const TRANSFER: u64 = knowledge::description(POWER_STONE, 1);
const RECIPIENT_HINT: u64 = knowledge::description(POWER_STONE, 2);

/// Partial rules only: this is not complete support for every listed card.
pub const SUPPORTED_CODES: &[u32] = &[
    POWER_STONE,
    CITADEL,
    LIBRARY,
    BREAKER,
    BLAST,
    CERBERUS,
    DEFENDER,
    DARK_RED,
];

fn traps_negated(ctx: &Ctx) -> bool {
    ctx.obs.cards.iter().any(|card| {
        card.at.location.is_field()
            && card.position.face_up
            && card
                .code
                .is_some_and(|code| knowledge::profile(ctx.canonical(code)).negates_field_traps)
            // Existing facts describe Jinzo's MZONE effect and Royal Decree's
            // SZONE effect. An equipped Jinzo has no live monster-zone aura.
            && match card.at.location {
                Location::MonsterZone => ctx.view_data(card).is_monster() && !ctx.effects_drained(),
                Location::SpellTrapZone => ctx.view_data(card).is_trap(),
                _ => false,
            }
    })
}

/// Marginal public usefulness of one transferred Spell Counter. Thresholds
/// refer only to the named card's own script. No hidden Deck target, opponent
/// hand identity, or legal future activation is inferred from them.
fn recipient_value(ctx: &Ctx, card: &CardView) -> f64 {
    if card.at.controller != ctx.me
        || !card.at.location.is_field()
        || !card.position.face_up
        || !ctx.reaches(card, POWER_STONE, true, false)
    {
        return 0.0;
    }
    let Some(code) = card.code.map(|code| ctx.canonical(code)) else {
        return 0.0;
    };
    if code == CITADEL {
        return if card.at.location == Location::SpellTrapZone && card.at.sequence == 5 {
            // c39910367: one counter can protect the bank from destruction;
            // remaining counters can replace another card's counter cost.
            if card.counters == 0 {
                180.0
            } else {
                60.0
            }
        } else {
            0.0
        };
    }
    if card.at.location != Location::MonsterZone || ctx.effects_drained() {
        return 0.0;
    }
    match code {
        // c70791313: limit three, remove three to draw one. Deck size is public;
        // no claim is made that a draw-negating effect cannot intervene.
        LIBRARY if card.counters < 3 && ctx.deck_size(ctx.me) > 0 => {
            if card.counters == 2 {
                1200.0
            } else {
                300.0 + 150.0 * f64::from(card.counters)
            }
        }
        // c71413901: limit one, spend one to destroy a targeted Spell/Trap;
        // its continuous +300 ATK is also useful without a current target.
        BREAKER if card.counters == 0 => {
            if ctx.obs.cards.iter().any(|target| {
                target.at.controller == ctx.opp
                    && target.at.location == Location::SpellTrapZone
                    && ctx.reaches(target, BREAKER, true, true)
            }) {
                1000.0
            } else {
                220.0
            }
        }
        // c21051146: ceil(current target ATK / 700), minimum one. Only known
        // face-up opponent monsters can justify a destruction threshold.
        BLAST => ctx
            .obs
            .cards
            .iter()
            .filter(|target| {
                target.at.controller == ctx.opp
                    && target.at.location == Location::MonsterZone
                    && target.position.face_up
                    && target.known()
                    && ctx.reaches(target, BLAST, true, true)
            })
            .map(|target| {
                let needed = ((i64::from(target.attack).max(1) + 699) / 700) as u32;
                if card.counters >= needed {
                    0.0
                } else if card.counters.saturating_add(1) >= needed {
                    1100.0
                } else {
                    240.0
                }
            })
            .fold(0.0, f64::max),
        // c55424270: +500 ATK per counter; battle-end clears them after this
        // monster battled, so do not replenish it late in the same Battle Phase.
        CERBERUS
            if card.position.attack
                && (card.battles == 0 || !ctx.phase().is_some_and(Phase::is_battle)) =>
        {
            350.0
        }
        // c2525268: limit one. Destruction protection can also spend counters
        // from another own card, so prefer other immediate payoff thresholds.
        DEFENDER if card.counters == 0 => 160.0,
        // c45462639: remove two for one random opponent discard, once per
        // turn; hand size, unlike its contents, is public. Also +300 ATK each.
        DARK_RED => {
            if ctx.hand_size(ctx.opp) > 0 && card.counters < 2 {
                if card.counters == 1 {
                    1050.0
                } else {
                    280.0
                }
            } else if card.position.attack {
                200.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

fn has_recipient(t: &Turn, source: CardRef) -> bool {
    t.ctx
        .obs
        .cards
        .iter()
        .any(|card| card.at != source && recipient_value(&t.ctx, card) > 0.0)
}

fn bank_useful(t: &Turn, source: CardRef) -> bool {
    has_recipient(t, source)
        // Defender can spend a counter from anywhere on our field, including
        // this bank, even when Defender's own one-counter capacity is full.
        || !t.ctx.effects_drained() && t.ctx.obs.cards.iter().any(|card| {
            card.at.controller == t.ctx.me && card.at.location == Location::MonsterZone
                && card.position.face_up && t.ctx.is(card, DEFENDER)
        })
}

/// Exact Power Stone activation/transfer only. `true` needs a positive main
/// and chain policy response; merely allowing the generic Trap fallback would
/// still skip this CATEGORY_COUNTER effect. Unrelated descriptions are unknown.
pub(super) fn allow_activation(t: &Turn, choice: &Choice) -> Option<bool> {
    if choice.kind != ChoiceKind::Activate
        || choice.code().map(|code| t.ctx.canonical(code)) != Some(POWER_STONE)
        || !t.decision.choices.contains(choice)
    {
        return None;
    }
    let at = choice.at()?;
    if at.controller != t.ctx.me || at.location != Location::SpellTrapZone || at.sequence >= 5 {
        return None;
    }
    let source = t.ctx.card(at)?;
    if !t.ctx.is(source, POWER_STONE) {
        return None;
    }
    let install = match choice.description {
        0 if !source.position.face_up => true,
        TRANSFER if source.position.face_up => false,
        _ => return None,
    };
    Some(
        t.ctx.my_turn()
            && !t.own_top()
            && !t.ctx.wasted(POWER_STONE)
            && !traps_negated(&t.ctx)
            && if install {
                bank_useful(t, at)
            } else {
                has_recipient(t, at)
            },
    )
}

/// Place Citadel before another currently offered Spell activation. The
/// already-live field card is preserved: this ordering rule does not justify
/// replacing an existing field investment or activating a face-up effect.
pub(super) fn activate_before_development(t: &Turn, choice: &Choice) -> bool {
    let ctx = t.ctx;
    if t.decision.kind != DecisionKind::Idle
        || !ctx.my_turn()
        || !matches!(ctx.phase(), Some(Phase::Main1 | Phase::Main2))
        || !ctx.obs.chain.is_empty()
        || choice.kind != ChoiceKind::Activate
        || choice.description != 0
        || choice.code().map(|code| ctx.canonical(code)) != Some(CITADEL)
        || !t.decision.choices.contains(choice)
        || ctx.wasted(CITADEL)
    {
        return false;
    }
    let Some(at) = choice.at().filter(|at| {
        at.controller == ctx.me
            && (at.location == Location::Hand
                || at.location == Location::SpellTrapZone && at.sequence == 5)
    }) else {
        return false;
    };
    let Some(source) = ctx.card(at).filter(|card| ctx.is(card, CITADEL)) else {
        return false;
    };
    if source.at.location.is_field() && source.position.face_up {
        return false;
    }
    if ctx.obs.cards.iter().any(|card| {
        card.at != at
            && card.at.controller == ctx.me
            && card.at.location == Location::SpellTrapZone
            && card.at.sequence == 5
    }) {
        return false;
    }
    t.choices().any(|(_, other)| {
        other.kind == ChoiceKind::Activate
            && other.at() != Some(at)
            && other.at().is_some_and(|at| {
                at.controller == ctx.me
                    && matches!(at.location, Location::Hand | Location::SpellTrapZone)
            })
            && other
                .code()
                .is_some_and(|code| ctx.data(code).is(types::SPELL))
            && other
                .at()
                .and_then(|at| ctx.card(at))
                .is_some_and(|card| card.at.location == Location::Hand || !card.position.face_up)
    })
}

/// Native c34029630.target uses SelectEffectYesNo(..., 94). The transfer is
/// fixed at one counter; there is no counter-amount option to implement.
pub(super) fn yes_no(t: &Turn) -> Option<bool> {
    if t.decision.kind != DecisionKind::YesNo
        || t.decision.subject.map(|code| t.ctx.canonical(code)) != Some(POWER_STONE)
    {
        return None;
    }
    let choice = t.decision.choices.iter().find(|choice| {
        choice.kind == ChoiceKind::Yes
            && choice.description == 94
            && choice.code().map(|code| t.ctx.canonical(code)) == Some(POWER_STONE)
    })?;
    let at = choice.at().filter(|at| {
        at.controller == t.ctx.me && at.location == Location::SpellTrapZone && at.sequence < 5
    })?;
    Some(
        t.ctx.my_turn()
            && !traps_negated(&t.ctx)
            && !t.ctx.wasted(POWER_STONE)
            && has_recipient(t, at),
    )
}

/// Match the script's custom target hint, not a generic hostile Target prompt.
/// It allows either field. Opponent cards must never win this beneficial target
/// ranking, including when the same recipient identity is present on both sides.
pub(super) fn select(t: &Turn, member: &Member) -> Option<f64> {
    if !matches!(
        t.decision.kind,
        DecisionKind::SelectCards | DecisionKind::SelectToggle
    ) || t.decision.hint != Hint::Other(RECIPIENT_HINT)
        || t.memory.last_activated.map(|code| t.ctx.canonical(code)) != Some(POWER_STONE)
    {
        return None;
    }
    if member.at.controller != t.ctx.me {
        return Some(-10_000.0);
    }
    let score = member
        .code
        .and_then(|code| {
            t.ctx
                .card(member.at)
                .filter(|card| t.ctx.is(card, t.ctx.canonical(code)))
        })
        .map_or(0.0, |card| recipient_value(&t.ctx, card));
    Some(if score > 0.0 {
        10_000.0 + score
    } else {
        -1000.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, Memory, Policy};
    use crate::cards::{CardData, MemoryCards};
    use crate::generic::Generic;
    use crate::model::{Decision, Observation, Position};
    use std::sync::Arc;

    const SWORDS: u32 = 72302403;
    const ROYAL_DECREE: u32 = 51452091;
    const JINZO: u32 = 77585513;
    const SKILL_DRAIN: u32 = 82732705;

    fn database() -> MemoryCards {
        let mut cards = MemoryCards::default();
        for code in
            SUPPORTED_CODES
                .iter()
                .copied()
                .chain([SWORDS, ROYAL_DECREE, JINZO, SKILL_DRAIN])
        {
            let kind = match code {
                POWER_STONE | ROYAL_DECREE | SKILL_DRAIN => types::TRAP | types::CONTINUOUS,
                CITADEL => types::SPELL | types::FIELD,
                SWORDS => types::SPELL,
                _ => types::MONSTER | types::EFFECT,
            };
            cards.0.insert(
                code,
                CardData {
                    code,
                    kind,
                    attack: 1400,
                    defense: 1000,
                    level: 4,
                    ..CardData::default()
                },
            );
        }
        cards
    }

    fn view(code: u32, location: Location, sequence: u32) -> CardView {
        CardView {
            at: CardRef {
                controller: 0,
                location,
                sequence,
            },
            code: Some(code),
            position: Position::FACE_UP_ATTACK,
            attack: 1400,
            defense: 1000,
            level: 4,
            can_attack: true,
            battles: 0,
            counters: 0,
            coin_effect: None,
        }
    }
    fn member(card: &CardView) -> Member {
        Member {
            at: card.at,
            code: card.code,
            value: 0,
            required: false,
        }
    }
    fn choice(kind: ChoiceKind, card: &CardView, description: u64) -> Choice {
        Choice {
            kind,
            card: Some(member(card)),
            members: vec![],
            description,
            place: None,
        }
    }
    fn plain(kind: ChoiceKind) -> Choice {
        Choice {
            kind,
            card: None,
            members: vec![],
            description: 0,
            place: None,
        }
    }
    fn prompt(kind: DecisionKind, choices: Vec<Choice>) -> Decision {
        Decision {
            kind,
            hint: Hint::None,
            minimum: 0,
            maximum: 1,
            selected: vec![],
            subject: None,
            choices,
        }
    }
    fn observation(cards: Vec<CardView>) -> Observation {
        Observation {
            me: 0,
            turn: 4,
            turn_player: Some(0),
            phase: Some(Phase::Main1),
            life_points: [8000, 8000],
            cards,
            pile_sizes: vec![(0, Location::Deck, 20), (1, Location::Hand, 2)],
            chain: vec![],
            battle_attacker: None,
            battle_target: None,
            event_cards: vec![],
            summon_used: true,
            chain_known: true,
            can_attack_known: true,
            coin_toss: None,
        }
    }
    fn with_turn<R>(obs: &Observation, decision: &Decision, f: impl FnOnce(&Turn) -> R) -> R {
        let db = database();
        let mut memory = Memory::default();
        f(&Turn {
            ctx: Ctx::new(obs, &db),
            decision,
            memory: &mut memory,
        })
    }
    fn bank() -> CardView {
        CardView {
            position: Position::FACE_DOWN_DEFENSE,
            ..view(POWER_STONE, Location::SpellTrapZone, 1)
        }
    }

    #[test]
    fn native_bank_question_and_target_pipeline_selects_own_threshold() {
        let source = bank();
        let mut ours = view(LIBRARY, Location::MonsterZone, 0);
        ours.counters = 2;
        let mut enemy = ours.clone();
        enemy.at.controller = 1;
        let mut obs = observation(vec![source.clone(), ours.clone(), enemy.clone()]);
        let mut agent = Agent::new(Generic, Arc::new(database()));
        let activate = prompt(
            DecisionKind::Idle,
            vec![
                plain(ChoiceKind::EndTurn),
                choice(ChoiceKind::Activate, &source, 0),
            ],
        );
        assert_eq!(agent.choose(&obs, &activate), 1);
        // c34029630.target places three counters before asking whether to
        // transfer one. Native SelectEffectYesNo uses literal description94.
        obs.cards[0].position = Position::FACE_UP_ATTACK;
        obs.cards[0].counters = 3;
        let mut question = prompt(
            DecisionKind::YesNo,
            vec![
                choice(ChoiceKind::No, &source, 94),
                choice(ChoiceKind::Yes, &source, 94),
            ],
        );
        question.subject = Some(POWER_STONE);
        assert_eq!(agent.choose(&obs, &question), 1);
        // Native HINT_SELECTMSG is Stringid(card,2), not HINTMSG_TARGET.
        let mut targets = prompt(
            DecisionKind::SelectCards,
            vec![
                Choice {
                    members: vec![member(&enemy)],
                    ..plain(ChoiceKind::Cards)
                },
                Choice {
                    members: vec![member(&ours)],
                    ..plain(ChoiceKind::Cards)
                },
            ],
        );
        targets.minimum = 1;
        targets.hint = Hint::Other(35_682_653_306_882);
        assert_eq!(agent.choose(&obs, &targets), 1);
    }

    #[test]
    fn native_chain_transfer_is_productive_only_for_own_live_recipient() {
        let mut source = bank();
        source.position = Position::FACE_UP_ATTACK;
        source.counters = 2;
        let mut library = view(LIBRARY, Location::MonsterZone, 0);
        library.counters = 2;
        let mut obs = observation(vec![source.clone(), library]);
        let chain = prompt(
            DecisionKind::Chain {
                forced: false,
                triggers: false,
            },
            vec![
                plain(ChoiceKind::Pass),
                choice(ChoiceKind::Activate, &source, 35_682_653_306_881),
            ],
        );
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &chain),
            1
        );
        obs.cards[1].at.controller = 1;
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &chain),
            0
        );
        obs.cards[1].at.controller = 0;
        obs.cards[1].position = Position::FACE_DOWN_DEFENSE;
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &chain),
            0
        );
        obs.cards[1].position = Position::FACE_UP_ATTACK;
        obs.turn_player = Some(1);
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &chain),
            0
        );
    }

    #[test]
    fn power_stone_does_not_claim_other_effect_descriptions_or_questions() {
        let source = bank();
        let obs = observation(vec![
            source.clone(),
            view(LIBRARY, Location::MonsterZone, 0),
        ]);
        for description in [
            knowledge::description(POWER_STONE, 0),
            TRANSFER,
            RECIPIENT_HINT,
            94,
        ] {
            let decision = prompt(
                DecisionKind::Idle,
                vec![choice(ChoiceKind::Activate, &source, description)],
            );
            assert_eq!(
                with_turn(&obs, &decision, |t| allow_activation(
                    t,
                    &decision.choices[0]
                )),
                None
            );
        }
        for description in [0, 95, 96, TRANSFER] {
            let mut question = prompt(
                DecisionKind::YesNo,
                vec![choice(ChoiceKind::Yes, &source, description)],
            );
            question.subject = Some(POWER_STONE);
            assert_eq!(with_turn(&obs, &question, yes_no), None);
        }
        let decision = prompt(DecisionKind::Idle, vec![]);
        assert_eq!(
            with_turn(&obs, &decision, |t| allow_activation(
                t,
                &choice(ChoiceKind::Activate, &source, 0)
            )),
            None
        );
    }

    #[test]
    fn depleted_deck_and_unknown_opponent_stats_do_not_create_payoffs() {
        let db = database();
        let mut library = view(LIBRARY, Location::MonsterZone, 0);
        library.counters = 2;
        let blast = view(BLAST, Location::MonsterZone, 1);
        let mut enemy = view(BREAKER, Location::MonsterZone, 0);
        enemy.at.controller = 1;
        enemy.code = None;
        enemy.position = Position::FACE_DOWN_DEFENSE;
        enemy.attack = 100;
        let mut obs = observation(vec![library.clone(), blast.clone(), enemy]);
        obs.pile_sizes
            .retain(|(_, location, _)| *location != Location::Deck);
        assert_eq!(recipient_value(&Ctx::new(&obs, &db), &library), 0.0);
        assert_eq!(recipient_value(&Ctx::new(&obs, &db), &blast), 0.0);
        obs.cards[2].attack = 9999;
        assert_eq!(recipient_value(&Ctx::new(&obs, &db), &blast), 0.0);
        obs.cards[2].position = Position::FACE_UP_ATTACK;
        obs.cards[2].code = Some(BREAKER);
        obs.cards[2].attack = 700;
        let next_counter = recipient_value(&Ctx::new(&obs, &db), &blast);
        obs.cards[2].attack = 701;
        assert!(next_counter > recipient_value(&Ctx::new(&obs, &db), &blast));
    }

    #[test]
    fn transfer_thresholds_prefer_immediate_draw_and_do_not_overfill_caps() {
        let db = database();
        let mut library = view(LIBRARY, Location::MonsterZone, 0);
        let breaker = view(BREAKER, Location::MonsterZone, 1);
        let citadel = view(CITADEL, Location::SpellTrapZone, 5);
        let mut enemy = view(SWORDS, Location::SpellTrapZone, 0);
        enemy.at.controller = 1;
        let obs = observation(vec![
            library.clone(),
            breaker.clone(),
            citadel.clone(),
            enemy,
        ]);
        let ctx = Ctx::new(&obs, &db);
        assert!(recipient_value(&ctx, &breaker) > recipient_value(&ctx, &library));
        library.counters = 2;
        assert!(recipient_value(&ctx, &library) > recipient_value(&ctx, &breaker));
        assert!(recipient_value(&ctx, &library) > recipient_value(&ctx, &citadel));
        library.counters = 3;
        assert_eq!(recipient_value(&ctx, &library), 0.0);
        let mut full_breaker = breaker;
        full_breaker.counters = 1;
        assert_eq!(recipient_value(&ctx, &full_breaker), 0.0);
    }

    #[test]
    fn bank_can_fund_defender_without_forcing_an_enemy_transfer() {
        let source = bank();
        let mut defender = view(DEFENDER, Location::MonsterZone, 0);
        defender.counters = 1;
        let mut enemy = view(LIBRARY, Location::MonsterZone, 0);
        enemy.at.controller = 1;
        let obs = observation(vec![source.clone(), defender, enemy]);
        let activation = prompt(
            DecisionKind::Idle,
            vec![choice(ChoiceKind::Activate, &source, 0)],
        );
        assert_eq!(
            with_turn(&obs, &activation, |t| allow_activation(
                t,
                &activation.choices[0]
            )),
            Some(true)
        );
        let mut question = prompt(
            DecisionKind::YesNo,
            vec![
                choice(ChoiceKind::Yes, &source, 94),
                choice(ChoiceKind::No, &source, 94),
            ],
        );
        question.subject = Some(POWER_STONE);
        assert_eq!(with_turn(&obs, &question, yes_no), Some(false));
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &question),
            1
        );
    }

    #[test]
    fn public_trap_negation_and_skill_drain_remove_false_counter_payoffs() {
        let source = bank();
        let library = view(LIBRARY, Location::MonsterZone, 0);
        let activation = prompt(
            DecisionKind::Idle,
            vec![choice(ChoiceKind::Activate, &source, 0)],
        );
        for (code, location) in [
            (ROYAL_DECREE, Location::SpellTrapZone),
            (JINZO, Location::MonsterZone),
            (SKILL_DRAIN, Location::SpellTrapZone),
        ] {
            for controller in [0, 1] {
                let mut lock = view(code, location, 4);
                lock.at.controller = controller;
                let obs = observation(vec![source.clone(), library.clone(), lock]);
                assert_eq!(
                    with_turn(&obs, &activation, |t| allow_activation(
                        t,
                        &activation.choices[0]
                    )),
                    Some(false),
                    "{code} controller{controller}"
                );
            }
        }
        for controller in [0, 1] {
            let mut equipped_jinzo = view(JINZO, Location::SpellTrapZone, 4);
            equipped_jinzo.at.controller = controller;
            let obs = observation(vec![source.clone(), library.clone(), equipped_jinzo]);
            assert_eq!(
                with_turn(&obs, &activation, |t| allow_activation(
                    t,
                    &activation.choices[0]
                )),
                Some(true)
            );
        }
    }

    #[test]
    fn citadel_precedes_swords_in_actual_agent_without_replacing_a_field() {
        let citadel = view(CITADEL, Location::Hand, 1);
        let swords = view(SWORDS, Location::Hand, 0);
        let mut obs = observation(vec![swords.clone(), citadel.clone()]);
        let decision = prompt(
            DecisionKind::Idle,
            vec![
                choice(ChoiceKind::Activate, &swords, 0),
                choice(ChoiceKind::Activate, &citadel, 0),
                plain(ChoiceKind::EndTurn),
            ],
        );
        assert_eq!(
            Agent::new(Generic, Arc::new(database())).choose(&obs, &decision),
            1
        );
        let mut existing = view(CITADEL, Location::SpellTrapZone, 5);
        existing.counters = 4;
        obs.cards.push(existing);
        assert!(!with_turn(
            &obs,
            &decision,
            |t| activate_before_development(t, &decision.choices[1])
        ));
        obs.cards.pop();
        // Activating a face-up Continuous Spell's effect is not activating a
        // Spell Card and therefore cannot be credited to the Citadel sequence.
        obs.cards[0].at.location = Location::SpellTrapZone;
        let other_effect = prompt(
            DecisionKind::Idle,
            vec![
                choice(ChoiceKind::Activate, &obs.cards[0], 0),
                decision.choices[1].clone(),
            ],
        );
        assert!(!with_turn(&obs, &other_effect, |t| {
            activate_before_development(t, &other_effect.choices[1])
        }));
    }

    #[test]
    fn recipient_rule_uses_native_hint_and_source_not_stale_activation_alone() {
        let ours = view(LIBRARY, Location::MonsterZone, 0);
        let obs = observation(vec![ours.clone()]);
        let db = database();
        let mut memory = Memory::default();
        memory.last_activated = Some(POWER_STONE);
        let mut decision = prompt(DecisionKind::SelectCards, vec![]);
        for hint in [Hint::Target, Hint::FaceUp, Hint::Other(TRANSFER)] {
            decision.hint = hint;
            let t = Turn {
                ctx: Ctx::new(&obs, &db),
                decision: &decision,
                memory: &mut memory,
            };
            assert_eq!(select(&t, &member(&ours)), None);
        }
        decision.hint = Hint::Other(RECIPIENT_HINT);
        memory.last_activated = Some(CITADEL);
        let t = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &decision,
            memory: &mut memory,
        };
        assert_eq!(select(&t, &member(&ours)), None);
    }
}
