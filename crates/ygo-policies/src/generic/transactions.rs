//! Narrow resource guards for known card transactions, using only public state.
//! These are card rules and reusable exchange tests, never deck dispatch.
//!
//! Sources: `vendor/CardScripts/official/c35480699.lua` (Eclipse),
//! `c10026986.lua` (King), `c73580471.lua` (Black Rose), and the
//! mandatory-removal facts in `knowledge`.
//! An ordinary NormalSummon choice does not expose its actual tribute cost.
//! Consequently this module never vetoes a summon from printed Level alone.

use super::{knowledge, Generic};
use crate::agent::{self, Outcome, Turn};
use crate::cards::{attributes, races, types};
use crate::model::{CardView, Choice, ChoiceKind, DecisionKind, Hint, Location, Member};
use crate::tactics;

const BOOK_OF_ECLIPSE: u32 = 35480699;
const WORM_KING: u32 = 10026986;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const WORM_SET: u16 = 0x3e;

/// Partial transaction facts only. Every identity occurs in the active pool.
pub const SUPPORTED_CODES: &[u32] = &[
    9748752,
    WORM_KING,
    BOOK_OF_ECLIPSE,
    73125233,
    BLACK_ROSE_DRAGON,
];

fn keep_value(t: &Turn, card: &CardView) -> i32 {
    agent::value(&Generic, &t.ctx, card.code, Some(card))
}

fn black_rose_source(t: &Turn, choice: &Choice) -> bool {
    choice.code().map(|code| t.ctx.canonical(code)) == Some(BLACK_ROSE_DRAGON)
        && choice
            .at()
            .is_some_and(|at| at.controller == t.ctx.me && at.location == Location::MonsterZone)
        && t.view(choice)
            .is_some_and(|card| card.position.face_up && t.ctx.is(card, BLACK_ROSE_DRAGON))
}

/// A conservative exchange estimate, not a simulator of destruction triggers.
/// Unknown opposing facedowns retain the public uncertainty values supplied
/// by sweep_worth. Own cards are charged their full keep value: a generic GY
/// payoff is not guaranteed when destruction/replacement conditions are opaque.
fn global_destruction_profitable(t: &Turn, source: u32) -> bool {
    let mut removed = 0i64;
    let mut lost = 0i64;
    for card in t
        .ctx
        .obs
        .cards
        .iter()
        .filter(|card| card.at.location.is_field())
    {
        if !t.ctx.reaches(card, source, false, true) {
            continue;
        }
        if card.at.controller == t.ctx.me {
            lost += i64::from(keep_value(t, card).max(0));
        } else if card.at.controller == t.ctx.opp {
            let worth = if card.at.location == Location::SpellTrapZone
                && card.position.face_up
                && t.ctx.is_toon_world(card)
            {
                // The shared single-card removal score includes dependent
                // Toon bodies. This sweep already counts each body below.
                card.code
                    .map_or(0, |code| {
                        crate::knowledge::owner_worth(t.ctx.canonical(code))
                    })
                    .max(900)
            } else {
                t.ctx.sweep_worth(card)
            };
            removed += i64::from(worth.max(0));
        }
    }
    removed > lost
}

/// c73580471.descon offers only the optional Synchro-summon destruction
/// trigger. Its operation destroys ALL field cards, including Black Rose.
/// The Plant-banish ignition has description 1 and must not inherit this rule.
pub(super) fn yes_no(t: &Turn) -> Option<bool> {
    if t.decision.kind != DecisionKind::YesNo
        || t.decision.subject.map(|code| t.ctx.canonical(code)) != Some(BLACK_ROSE_DRAGON)
    {
        return None;
    }
    let choice = t
        .decision
        .choices
        .iter()
        .find(|choice| choice.kind == ChoiceKind::Yes)?;
    // OCGCore processor.cpp's single optional-trigger paths emit 0 or 221,
    // not the effect description. Other generic questions (e.g. direct attack
    // 31 or replacement 94) must retain their original behavior.
    if !black_rose_source(t, choice)
        || !(matches!(choice.description, 0 | 221)
            || choice.description == knowledge::description(BLACK_ROSE_DRAGON, 0))
    {
        return None;
    }
    Some(global_destruction_profitable(t, BLACK_ROSE_DRAGON))
}

fn can_be_set(t: &Turn, card: &CardView) -> bool {
    card.at.location == Location::MonsterZone
        && card.position.face_up
        && !t.ctx.view_data(card).is(types::TOKEN | types::LINK)
        && t.ctx.reaches(card, BOOK_OF_ECLIPSE, false, false)
}

fn eclipse(t: &Turn) -> Option<bool> {
    let ctx = t.ctx;
    let own: Vec<_> = ctx
        .monsters(ctx.me)
        .into_iter()
        .filter(|card| can_be_set(t, card))
        .collect();
    if own.is_empty() {
        return None;
    }
    // The normal categories do not state who draws. Eclipse sets both fields,
    // then flips only the opponent's facedowns and makes THEM draw at End Phase.
    // Do not spend it just to turn our own board face-down as a "draw" spell.
    if ctx.monsters(ctx.opp).iter().any(|card| can_be_set(t, card)) {
        return None;
    }

    // Leave actual defense and flip-reset opportunities to ordinary timing or
    // search. A generic FLIP monster can be useful without any card-id entry.
    if own.iter().any(|card| ctx.view_data(card).is(types::FLIP)) {
        return None;
    }
    if ctx
        .incoming_attack()
        .is_some_and(|(attacker, target)| ctx.attack_hurts(attacker, target))
    {
        return None;
    }
    if ctx.obs.chain.iter().any(|link| {
        link.controller == ctx.opp
            && link
                .targets
                .iter()
                .any(|at| own.iter().any(|card| card.at == *at))
    }) {
        return None;
    }
    // Already face-down opposing monsters also flip and draw. Preserve a
    // possible deck-out when that forced draw exceeds the public deck count.
    let opponent_facedowns = ctx
        .monsters(ctx.opp)
        .iter()
        .filter(|card| !card.position.face_up)
        .count();
    if opponent_facedowns > ctx.deck_size(ctx.opp) as usize {
        return None;
    }
    Some(false)
}

fn worm_tribute(t: &Turn, card: &CardView) -> bool {
    let data = t.ctx.view_data(card);
    card.at.location == Location::MonsterZone
        && data.in_set(WORM_SET)
        && data.race & races::REPTILE != 0
}

fn king(t: &Turn, choice: &Choice) -> Option<bool> {
    let ctx = t.ctx;
    let source = t.view(choice)?;
    if !ctx.main1()
        || !ctx.obs.chain.is_empty()
        || !source.position.face_up
        || !source.position.attack
        || !ctx.can_attack(source)
        || !ctx.attack_locks().is_empty()
    {
        return None;
    }
    // c10026986.descost can release King itself. Only reject the clean case
    // where every visible own eligible cost is the source. Cheap fodder remains
    // useful. A known earlier tribute-permission effect keeps an opposing Worm
    // as a possible cost; merely being a Worm does not make it releasable.
    let own_costs: Vec<_> = ctx
        .monsters(ctx.me)
        .into_iter()
        .filter(|card| worm_tribute(t, card))
        .collect();
    if own_costs.len() != 1 || own_costs[0].at != source.at {
        return None;
    }
    let possible_opponent_tribute = ctx
        .monsters(ctx.opp)
        .iter()
        .any(|card| card.position.face_up && worm_tribute(t, card))
        && t.memory.activated.iter().any(|code| {
            knowledge::action(ctx.canonical(*code), knowledge::ActionKind::Activation, 0)
                .is_some_and(|facts| {
                    facts.payoff == knowledge::Operation::UseTargetAsTributeThisTurn
                })
        });
    if possible_opponent_tribute {
        return None;
    }
    let enemies: Vec<_> = ctx
        .obs
        .cards
        .iter()
        .filter(|card| card.at.controller == ctx.opp && card.at.location.is_field())
        .collect();
    if enemies.len() != 1 {
        return None;
    }
    let target = enemies[0];
    if target.at.location != Location::MonsterZone
        || !target.position.face_up
        || !ctx.reaches(target, WORM_KING, true, true)
        || ctx.attack_negatable(target)
    {
        return None;
    }
    // Do not reject important engine threats, battle-proof monsters, or a
    // target whose battle-death payoff makes effect removal preferable.
    let met = ctx.attack_meets(Some(source), target);
    if ctx.battle_proof(target)
        || ctx.facts(target).negates_any != 0
        || ctx.threat(target) >= keep_value(t, source)
        || met.facts.payoff > 0
        || !matches!(
            tactics::default_outcome(&ctx, source, target, 0),
            Outcome::Win { trick: false }
        )
    {
        return None;
    }
    // Releasing King can still clear a blocker for lethal with OTHER bodies.
    let remaining_damage: i32 = ctx
        .monsters(ctx.me)
        .iter()
        .filter(|card| card.at != source.at && ctx.can_attack(card))
        .map(|card| card.attack.max(0))
        .sum();
    if remaining_damage >= ctx.opp_lp() {
        return None;
    }
    Some(false)
}

/// A decision only for a precisely identified transaction. `None` preserves
/// the generic policy's timing and the engine's legal options.
pub(super) fn allow_activation(t: &Turn, choice: &Choice) -> Option<bool> {
    if choice.kind != ChoiceKind::Activate || choice.at().is_none_or(|at| at.controller != t.ctx.me)
    {
        return None;
    }
    let code = t.ctx.canonical(choice.code()?);
    if black_rose_source(t, choice)
        && choice.description == knowledge::description(BLACK_ROSE_DRAGON, 0)
    {
        return Some(global_destruction_profitable(t, BLACK_ROSE_DRAGON));
    }
    if matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. }) {
        return None;
    }
    match code {
        BOOK_OF_ECLIPSE
            if choice.description == 0
                && matches!(
                    choice.at()?.location,
                    Location::Hand | Location::SpellTrapZone
                ) =>
        {
            eclipse(t)
        }
        WORM_KING
            if choice.description == knowledge::description(code, 1)
                && choice.at()?.location == Location::MonsterZone =>
        {
            king(t, choice)
        }
        _ => None,
    }
}

pub(super) fn select(t: &Turn, member: &Member) -> Option<f64> {
    let ctx = t.ctx;
    let source = ctx.canonical(t.memory.last_activated?);
    let card = ctx.card(member.at)?;
    if source == WORM_KING && t.decision.hint == Hint::Release && worm_tribute(t, card) {
        // If the engine actually offers an opponent's body, using it as the
        // cost is a gain. Own fodder is ranked by what we give up, not printed
        // ATK alone; unrelated material prompts never inherit this rule.
        return Some(if member.at.controller == ctx.me {
            -f64::from(keep_value(t, card))
        } else {
            10_000.0 + f64::from(ctx.threat(card))
        });
    }
    let removal = knowledge::profile(source).mandatory_removal?;
    let relevant = matches!(
        (removal.operation, t.decision.hint),
        (knowledge::RemovalOperation::TopOfDeck, Hint::ToDeck)
            | (knowledge::RemovalOperation::BanishFaceUp, Hint::Banish)
    );
    if !relevant || !member.at.location.is_field() {
        return None;
    }
    if removal.dark_monster_banish_damage > 0
        && ctx.opp_lp() <= i32::from(removal.dark_monster_banish_damage)
        && ctx.view_data(card).is_monster()
        // Tokens disappear instead of remaining banished; Caius checks that
        // the target is still in LOCATION_REMOVED before applying damage.
        && !ctx.view_data(card).is(types::TOKEN)
        && ctx.view_data(card).attribute & attributes::DARK != 0
        && ctx.reaches(card, source, true, false)
    {
        // c9748752.operation also deals the damage for our own banished DARK
        // monster, including Caius. Never price that winning target as a cost.
        return Some(100_000.0);
    }
    if member.at.controller != ctx.me {
        return None;
    }
    let mut loss = f64::from(keep_value(t, card));
    if removal.operation == knowledge::RemovalOperation::TopOfDeck
        && member
            .code
            .is_some_and(|code| ctx.canonical(code) == source)
    {
        // Returning the newly summoned body also consumes the next draw and
        // can repeat the same forced-removal cycle. Prefer expendable targets
        // when available; if it is the only target, the engine still requires it.
        loss += 1500.0;
    }
    Some(-loss)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{CardData, MemoryCards};
    use crate::ctx::Ctx;
    use crate::model::{CardRef, ChainLink, Decision, Observation, Phase, Position};

    const ORDINARY: u32 = 900_510_001;
    const SMALL: u32 = 900_510_002;
    const FLIP: u32 = 900_510_003;
    const FODDER: u32 = 900_510_004;
    const TOON_BODY: u32 = 900_510_006;

    fn db() -> MemoryCards {
        let cards = [
            CardData {
                code: BLACK_ROSE_DRAGON,
                kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 2400,
                defense: 1800,
                level: 7,
                ..CardData::default()
            },
            CardData {
                code: crate::knowledge::TOON_WORLD,
                kind: types::SPELL | types::CONTINUOUS,
                ..CardData::default()
            },
            CardData {
                code: TOON_BODY,
                kind: types::MONSTER | types::EFFECT,
                attack: 3000,
                level: 8,
                setcodes: vec![crate::knowledge::SET_TOON],
                ..CardData::default()
            },
            CardData {
                code: WORM_KING,
                kind: types::MONSTER | types::EFFECT,
                race: races::REPTILE,
                setcodes: vec![WORM_SET],
                attack: 2700,
                defense: 1100,
                level: 8,
                ..CardData::default()
            },
            CardData {
                code: BOOK_OF_ECLIPSE,
                kind: types::SPELL | types::QUICKPLAY,
                ..CardData::default()
            },
            CardData {
                code: ORDINARY,
                kind: types::MONSTER | types::EFFECT,
                attack: 1800,
                level: 4,
                ..CardData::default()
            },
            CardData {
                code: SMALL,
                kind: types::MONSTER,
                attack: 1200,
                defense: 500,
                level: 4,
                race: races::REPTILE,
                setcodes: vec![WORM_SET],
                ..CardData::default()
            },
            CardData {
                code: FLIP,
                kind: types::MONSTER | types::EFFECT | types::FLIP,
                attack: 500,
                level: 2,
                ..CardData::default()
            },
            CardData {
                code: FODDER,
                kind: types::MONSTER,
                attack: 100,
                level: 1,
                race: races::REPTILE,
                setcodes: vec![WORM_SET],
                ..CardData::default()
            },
            CardData {
                code: 900_510_005,
                kind: types::MONSTER | types::TOKEN,
                attribute: attributes::DARK,
                attack: 0,
                level: 1,
                ..CardData::default()
            },
            CardData {
                code: 9748752,
                kind: types::MONSTER | types::EFFECT,
                attribute: attributes::DARK,
                attack: 2400,
                defense: 1000,
                level: 6,
                ..CardData::default()
            },
            CardData {
                code: 73125233,
                kind: types::MONSTER | types::EFFECT,
                attribute: attributes::WIND,
                attack: 2400,
                defense: 1000,
                level: 6,
                ..CardData::default()
            },
            CardData {
                code: 23205979,
                kind: types::MONSTER | types::EFFECT,
                attack: 300,
                defense: 200,
                level: 3,
                ..CardData::default()
            },
        ];
        MemoryCards(cards.into_iter().map(|card| (card.code, card)).collect())
    }

    fn card(code: u32, controller: u8, sequence: u32) -> CardView {
        let db = db();
        let data = &db.0[&code];
        CardView {
            at: CardRef {
                controller,
                location: Location::MonsterZone,
                sequence,
            },
            code: Some(code),
            position: Position::FACE_UP_ATTACK,
            attack: data.attack,
            defense: data.defense,
            level: data.level,
            can_attack: true,
            battles: 0,
            counters: 0,
            coin_effect: None,
        }
    }

    fn obs(cards: Vec<CardView>) -> Observation {
        Observation {
            me: 0,
            turn: 2,
            turn_player: Some(0),
            phase: Some(Phase::Main1),
            life_points: [8000; 2],
            cards,
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

    fn member(card: &CardView) -> Member {
        Member {
            at: card.at,
            code: card.code,
            value: 0,
            required: false,
        }
    }

    fn activation(code: u32) -> Choice {
        Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member {
                at: CardRef {
                    controller: 0,
                    location: if code == WORM_KING {
                        Location::MonsterZone
                    } else {
                        Location::Hand
                    },
                    sequence: 0,
                },
                code: Some(code),
                value: 0,
                required: false,
            }),
            members: vec![],
            description: if code == WORM_KING {
                knowledge::description(code, 1)
            } else {
                0
            },
            place: None,
        }
    }

    fn with_turn<R>(
        obs: &Observation,
        hint: Hint,
        source: Option<u32>,
        f: impl FnOnce(&Turn) -> R,
    ) -> R {
        let db = db();
        let mut memory = Memory::default();
        memory.last_activated = source;
        let decision = Decision {
            kind: DecisionKind::Idle,
            hint,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: None,
            choices: vec![],
        };
        f(&Turn {
            ctx: Ctx::new(obs, &db),
            decision: &decision,
            memory: &mut memory,
        })
    }

    fn black_rose_prompt(yes_no_description: Option<u64>) -> Decision {
        let source = card(BLACK_ROSE_DRAGON, 0, 0);
        let choice = Choice {
            kind: if yes_no_description.is_some() {
                ChoiceKind::Yes
            } else {
                ChoiceKind::Activate
            },
            card: Some(member(&source)),
            members: vec![],
            description: yes_no_description
                .unwrap_or_else(|| knowledge::description(BLACK_ROSE_DRAGON, 0)),
            place: None,
        };
        let mut other = choice.clone();
        other.kind = if yes_no_description.is_some() {
            ChoiceKind::No
        } else {
            ChoiceKind::Pass
        };
        Decision {
            kind: if yes_no_description.is_some() {
                DecisionKind::YesNo
            } else {
                DecisionKind::Chain {
                    forced: false,
                    triggers: true,
                }
            },
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: yes_no_description.map(|_| BLACK_ROSE_DRAGON),
            choices: vec![choice, other],
        }
    }

    fn black_rose_answer(board: &Observation, decision: &Decision) -> Option<bool> {
        let db = db();
        let mut memory = Memory::default();
        let t = Turn {
            ctx: Ctx::new(board, &db),
            decision,
            memory: &mut memory,
        };
        if decision.kind == DecisionKind::YesNo {
            yes_no(&t)
        } else {
            allow_activation(&t, &decision.choices[0])
        }
    }

    #[test]
    fn black_rose_refuses_its_own_body_for_one_unknown_monster() {
        let mut unknown = card(SMALL, 1, 0);
        unknown.code = None;
        unknown.position = Position::FACE_DOWN_DEFENSE;
        unknown.attack = 0;
        unknown.defense = 0;
        let board = obs(vec![card(BLACK_ROSE_DRAGON, 0, 0), unknown]);
        for description in [0, 221, knowledge::description(BLACK_ROSE_DRAGON, 0)] {
            assert_eq!(
                black_rose_answer(&board, &black_rose_prompt(Some(description))),
                Some(false)
            );
        }
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(None)),
            Some(false)
        );
        let own_only = obs(vec![card(BLACK_ROSE_DRAGON, 0, 0)]);
        assert_eq!(
            black_rose_answer(&own_only, &black_rose_prompt(Some(221))),
            Some(false)
        );
    }

    #[test]
    fn black_rose_accepts_profitable_removal_but_counts_own_backrow() {
        let mut board = obs(vec![
            card(BLACK_ROSE_DRAGON, 0, 0),
            card(ORDINARY, 1, 0),
            card(ORDINARY, 1, 1),
        ]);
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(None)),
            Some(true)
        );
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(Some(221))),
            Some(true)
        );

        let mut backrow = card(BOOK_OF_ECLIPSE, 0, 0);
        backrow.at.location = Location::SpellTrapZone;
        backrow.position = Position::FACE_DOWN_DEFENSE;
        board.cards.push(backrow);
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(None)),
            Some(false)
        );
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(Some(221))),
            Some(false)
        );
    }

    #[test]
    fn black_rose_counts_unknown_backrow_without_using_concealed_stats() {
        let mut unknown = card(SMALL, 1, 0);
        unknown.code = None;
        unknown.position = Position::FACE_DOWN_DEFENSE;
        let mut backrow = unknown.clone();
        backrow.at.location = Location::SpellTrapZone;
        let mut other_backrow = backrow.clone();
        other_backrow.at.sequence = 1;
        let mut board = obs(vec![
            card(BLACK_ROSE_DRAGON, 0, 0),
            unknown,
            backrow,
            other_backrow,
        ]);
        let decision = black_rose_prompt(Some(221));
        assert_eq!(black_rose_answer(&board, &decision), Some(true));

        // Concealed field stats and off-field cards cannot affect this public
        // board exchange. In actual observations concealed stats are redacted.
        for card in &mut board.cards[1..] {
            card.attack = 100_000;
            card.defense = 100_000;
            card.level = 12;
            card.counters = 99;
        }
        let mut hand = card(BLACK_ROSE_DRAGON, 1, 0);
        hand.at.location = Location::Hand;
        hand.code = None;
        board.cards.push(hand);
        assert_eq!(black_rose_answer(&board, &decision), Some(true));
        board.cards.truncate(2);
        assert_eq!(black_rose_answer(&board, &decision), Some(false));
        board.cards[1].attack = 0;
        board.cards[1].defense = 0;
        assert_eq!(black_rose_answer(&board, &decision), Some(false));
    }

    #[test]
    fn black_rose_matches_only_its_global_destruction_effect() {
        let board = obs(vec![card(BLACK_ROSE_DRAGON, 0, 0)]);
        let mut trigger = black_rose_prompt(None);
        trigger.choices[0].description = knowledge::description(BLACK_ROSE_DRAGON, 1);
        assert_eq!(black_rose_answer(&board, &trigger), None);
        trigger.choices[0].description = 0;
        assert_eq!(black_rose_answer(&board, &trigger), None);
        for description in [31, 94, knowledge::description(BLACK_ROSE_DRAGON, 1)] {
            assert_eq!(
                black_rose_answer(&board, &black_rose_prompt(Some(description))),
                None
            );
        }
        let mut question = black_rose_prompt(Some(221));
        question.subject = Some(ORDINARY);
        assert_eq!(black_rose_answer(&board, &question), None);
        question.subject = Some(BLACK_ROSE_DRAGON);
        question.choices[0].card.as_mut().unwrap().at.controller = 1;
        assert_eq!(black_rose_answer(&board, &question), None);
    }

    #[test]
    fn black_rose_does_not_count_toon_collateral_twice() {
        let mut world = card(crate::knowledge::TOON_WORLD, 1, 0);
        world.at.location = Location::SpellTrapZone;
        let board = obs(vec![
            card(BLACK_ROSE_DRAGON, 0, 0),
            card(TOON_BODY, 0, 1),
            card(TOON_BODY, 1, 0),
            world,
        ]);
        // Our two bodies cost 2750 + 3350. Their World and body are 3000
        // each, not World + body + the same body's collateral value.
        assert_eq!(
            black_rose_answer(&board, &black_rose_prompt(Some(221))),
            Some(false)
        );
    }

    #[test]
    fn eclipse_does_not_mistake_opponent_draw_for_own_resources() {
        let board = obs(vec![card(ORDINARY, 0, 0)]);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(
                allow_activation(t, &activation(BOOK_OF_ECLIPSE)),
                Some(false)
            )
        });
        let board = obs(vec![card(ORDINARY, 0, 0), card(SMALL, 1, 0)]);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(BOOK_OF_ECLIPSE)), None)
        });
    }

    #[test]
    fn eclipse_keeps_flip_reset_defense_and_forced_deckout() {
        let board = obs(vec![card(FLIP, 0, 0)]);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(BOOK_OF_ECLIPSE)), None)
        });
        let mut board = obs(vec![card(ORDINARY, 0, 0)]);
        board.chain.push(ChainLink {
            code: 900_510_009,
            controller: 1,
            source: CardRef {
                controller: 1,
                location: Location::SpellTrapZone,
                sequence: 0,
            },
            targets: vec![board.cards[0].at],
        });
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(BOOK_OF_ECLIPSE)), None)
        });
        board.chain.clear();
        let mut enemy = card(SMALL, 1, 0);
        enemy.position = Position::FACE_DOWN_DEFENSE;
        enemy.code = None;
        board.cards.push(enemy);
        board.pile_sizes[1].2 = 0;
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(BOOK_OF_ECLIPSE)), None)
        });
    }

    #[test]
    fn king_keeps_its_body_for_a_safe_attack_but_uses_cheap_fodder() {
        let mut board = obs(vec![card(WORM_KING, 0, 0), card(SMALL, 1, 0)]);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(WORM_KING)), Some(false))
        });
        board.cards.push(card(FODDER, 0, 1));
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(WORM_KING)), None)
        });
        with_turn(&board, Hint::Release, Some(WORM_KING), |t| {
            assert!(
                select(t, &member(&board.cards[2])).unwrap()
                    > select(t, &member(&board.cards[0])).unwrap()
            );
        });
    }

    #[test]
    fn king_preserves_removal_for_battle_proof_targets_and_other_body_lethal() {
        let mut board = obs(vec![card(WORM_KING, 0, 0), card(23205979, 1, 0)]);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(WORM_KING)), None)
        });
        board.cards[1] = card(SMALL, 1, 0);
        board.cards.push(card(ORDINARY, 0, 1));
        board.life_points[1] = 1800;
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(WORM_KING)), None)
        });
        board.life_points[1] = 8000;
        board.cards[0].can_attack = false;
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &activation(WORM_KING)), None)
        });
    }

    #[test]
    fn mandatory_selection_preserves_caius_self_banish_lethal() {
        let mut board = obs(vec![card(9748752, 0, 0)]);
        board.life_points[1] = 1000;
        with_turn(&board, Hint::Banish, Some(9748752), |t| {
            assert_eq!(select(t, &member(&board.cards[0])), Some(100_000.0))
        });
        board.life_points[1] = 1001;
        with_turn(&board, Hint::Banish, Some(9748752), |t| {
            assert!(select(t, &member(&board.cards[0])).unwrap() < 0.0)
        });
        with_turn(&board, Hint::Tribute, Some(9748752), |t| {
            assert_eq!(select(t, &member(&board.cards[0])), None)
        });
    }

    #[test]
    fn a_dark_token_cannot_supply_caius_banish_damage() {
        let mut board = obs(vec![card(9748752, 0, 0), card(900_510_005, 0, 1)]);
        board.life_points[1] = 1000;
        with_turn(&board, Hint::Banish, Some(9748752), |t| {
            assert_eq!(select(t, &member(&board.cards[0])), Some(100_000.0));
            assert!(select(t, &member(&board.cards[1])).unwrap() < 100_000.0);
        });
    }

    #[test]
    fn mandatory_selection_avoids_recycling_raiza_when_fodder_is_offered() {
        let board = obs(vec![card(73125233, 0, 0), card(ORDINARY, 0, 1)]);
        with_turn(&board, Hint::ToDeck, Some(73125233), |t| {
            assert!(
                select(t, &member(&board.cards[1])).unwrap()
                    > select(t, &member(&board.cards[0])).unwrap()
            );
        });
        with_turn(&board, Hint::Release, Some(ORDINARY), |t| {
            assert_eq!(select(t, &member(&board.cards[0])), None)
        });
    }

    #[test]
    fn unrelated_actions_and_wrong_effect_slots_are_unchanged() {
        let board = obs(vec![card(WORM_KING, 0, 0), card(SMALL, 1, 0)]);
        let mut choice = activation(WORM_KING);
        choice.description = knowledge::description(WORM_KING, 0);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &choice), None)
        });
        choice.kind = ChoiceKind::NormalSummon;
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &choice), None)
        });
        choice = activation(ORDINARY);
        with_turn(&board, Hint::None, None, |t| {
            assert_eq!(allow_activation(t, &choice), None)
        });
    }
}
