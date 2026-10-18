//! Visible-card rules for collecting Exodia and resolving Destiny Board.
//!
//! These are properties of individual cards and their legal prompts. They do
//! not identify a deck or delegate to one of the specialist policies. Ordinary
//! attacks, summons and unrelated cards retain the generic policy's behavior.
use crate::agent::{Response, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{Choice, DecisionKind, Hint, Location, Member, Phase};

const PIECES: [u32; 5] = [33396948, 70903634, 7902349, 8124921, 44519536];
const HEART: u32 = 35762283;
const DUALITY: u32 = 98645731;
const RELOAD: u32 = 22589918;
const CARD_DESTRUCTION: u32 = 72892473;
const GOLD_SARCOPHAGUS: u32 = 75500286;
const BOARD: u32 = 94212438;
const LETTERS: [u32; 4] = [31893528, 67287533, 94772232, 30170981];
const PEACE: u32 = 44656491;
const GIANT_TRUNADE: u32 = crate::staples::GIANT_TRUNADE;

/// Identities with explicit contextual handling below. This records partial
/// semantic coverage; it is not a claim to model every effect of these cards.
pub const SUPPORTED_CODES: &[u32] = &[
    PIECES[0],
    PIECES[1],
    PIECES[2],
    PIECES[3],
    PIECES[4],
    HEART,
    DUALITY,
    BOARD,
    LETTERS[0],
    LETTERS[1],
    LETTERS[2],
    LETTERS[3],
    RELOAD,
    CARD_DESTRUCTION,
    GOLD_SARCOPHAGUS,
    PEACE,
    GIANT_TRUNADE,
];

/// Unique Exodia name, for state evaluation as well as hand preservation.
/// Callers should canonicalize alternate-art aliases through their database.
pub fn exodia_piece(code: u32) -> Option<usize> {
    PIECES.iter().position(|&piece| piece == code)
}

/// Heart's native initial script has no DRAW category on its trigger.
pub fn heart_of_the_underdog(code: u32) -> bool {
    code == HEART
}

pub fn destiny_board(code: u32) -> bool {
    code == BOARD
}

pub fn spirit_message(code: u32) -> bool {
    LETTERS.contains(&code)
}

fn copies_in_hand(ctx: &Ctx, code: u32) -> usize {
    ctx.obs
        .pile(ctx.me, Location::Hand)
        .filter(|card| ctx.is(card, code))
        .count()
}

fn unselected_hand_copies(t: &Turn, code: u32) -> usize {
    let selected = t
        .decision
        .selected
        .iter()
        .filter(|at| {
            at.controller == t.ctx.me
                && at.location == Location::Hand
                && t.ctx.card(**at).is_some_and(|card| t.ctx.is(card, code))
        })
        .count();
    copies_in_hand(&t.ctx, code).saturating_sub(selected)
}

fn distinct_pieces(ctx: &Ctx) -> usize {
    PIECES
        .iter()
        .filter(|&&code| copies_in_hand(ctx, code) > 0)
        .count()
}

fn board_visible(ctx: &Ctx) -> bool {
    ctx.obs.cards.iter().any(|card| {
        card.at.controller == ctx.me
            && ctx.is(card, BOARD)
            && matches!(card.at.location, Location::Hand | Location::SpellTrapZone)
    })
}

fn board_active(ctx: &Ctx) -> bool {
    ctx.face_up_on_field(ctx.me, BOARD)
}

fn letters_on_field(ctx: &Ctx) -> usize {
    LETTERS
        .iter()
        .filter(|&&code| ctx.face_up_on_field(ctx.me, code))
        .count()
}

fn used_spell_zones(ctx: &Ctx) -> usize {
    // The Field Zone is represented alongside Spell/Trap cards, but does not
    // consume one of the five spaces needed for the Board and its messages.
    ctx.obs
        .pile(ctx.me, Location::SpellTrapZone)
        .filter(|card| card.at.sequence < 5)
        .count()
}

fn hearts(ctx: &Ctx) -> usize {
    ctx.obs
        .pile(ctx.me, Location::SpellTrapZone)
        .filter(|card| card.position.face_up && ctx.is(card, HEART))
        .count()
}

fn permanent_backrow(ctx: &Ctx, code: u32) -> bool {
    let data = ctx.data(code);
    // A separate Field Zone does not obstruct messages. Messenger of Peace
    // can be removed by declining its own upkeep before the final message.
    code != BOARD
        && code != PEACE
        && !data.is(types::FIELD)
        && (data.is_spell() || data.is_trap())
        && data.is(types::CONTINUOUS | types::EQUIP)
}

fn safe_heart_draw(t: &Turn) -> bool {
    let pending = t
        .ctx
        .obs
        .chain
        .iter()
        .filter(|link| link.controller == t.ctx.me && t.ctx.canonical(link.code) == HEART)
        .count() as u32;
    t.ctx.my_turn() && t.ctx.phase() == Some(Phase::Draw) && t.ctx.deck_size(t.ctx.me) > pending
}

fn reload_heart(t: &Turn) -> bool {
    // Reload restarts Heart's engine in the Draw Phase (each Normal Monster
    // it draws there draws again), and digs for a Heart when none is out or in
    // hand, as the Exodia specialist plays it. Shuffling away four assembled
    // names is a poor fallback for drawing the missing fifth.
    let ctx = t.ctx;
    let digging = ctx.my_turn() && ctx.obs.chain.is_empty() && distinct_pieces(&ctx) < 4 && ctx.hand_size(ctx.me) >= 3;
    digging && match ctx.phase() {
        Some(Phase::Draw) => hearts(&ctx) > 0,
        Some(Phase::Main1 | Phase::Main2) => hearts(&ctx) == 0 && copies_in_hand(&ctx, HEART) == 0,
        _ => false,
    }
}

pub(super) fn value(ctx: &Ctx, code: u32) -> Option<i32> {
    let code = ctx.canonical(code);
    if PIECES.contains(&code) {
        return Some(if copies_in_hand(ctx, code) > 1 {
            1200
        } else {
            35_000
        });
    }
    match code {
        HEART => Some(if hearts(ctx) < 2 { 7000 } else { 4500 }),
        BOARD => Some(if board_active(ctx) {
            40_000 + letters_on_field(ctx) as i32 * 10_000
        } else {
            9000
        }),
        code if LETTERS.contains(&code) => {
            Some(if ctx.face_up_on_field(ctx.me, code) && board_active(ctx) {
                40_000 + letters_on_field(ctx) as i32 * 10_000
            } else if board_visible(ctx) {
                6000
            } else {
                1200
            })
        }
        _ => None,
    }
}

/// Preserve the last hand copy. An extra copy has no extra victory value and
/// can still be used as a body, material or cost in an unfamiliar combination.
pub(super) fn summon_score(t: &Turn, choice: &Choice) -> Option<Option<f64>> {
    let code = t.ctx.canonical(choice.code()?);
    (PIECES.contains(&code)
        && choice
            .at()
            .is_some_and(|at| at.controller == t.ctx.me && at.location == Location::Hand)
        && copies_in_hand(&t.ctx, code) <= 1)
        .then_some(None)
}

pub(super) fn special_summon(t: &Turn, choice: &Choice) -> Option<bool> {
    summon_score(t, choice).map(|_| false)
}

/// Duality reveals three cards and adds one (`c98645731.lua`). Establishing
/// the first Heart can make future Normal-Monster draws repeat in the Draw
/// Phase (`c35762283.lua`). This is a selection-order prior, not a claim
/// about the unseen Deck: require visible support and a real offered choice.
fn first_heart_from_duality(t: &Turn) -> bool {
    let ctx = t.ctx;
    if t.decision.kind != DecisionKind::SelectCards
        || t.decision.hint != Hint::AddToHand
        || t.decision.minimum != 1
        || t.decision.maximum != 1
        || t.memory.last_activated.map(|code| ctx.canonical(code)) != Some(DUALITY)
        || !ctx.obs.chain_known
        || !ctx
            .obs
            .chain
            .last()
            .is_some_and(|link| link.controller == ctx.me && ctx.canonical(link.code) == DUALITY)
        || !ctx.my_turn()
        || !matches!(ctx.phase(), Some(Phase::Main1 | Phase::Main2))
        || distinct_pieces(&ctx) >= 4
        || used_spell_zones(&ctx) >= 5
        || ctx.deck_size(ctx.me) <= 1
        || board_visible(&ctx)
        || ctx.wasted(HEART)
        || ctx.obs.cards.iter().any(|card| {
            card.at.controller == ctx.me
                && matches!(card.at.location, Location::Hand | Location::SpellTrapZone)
                && ctx.is(card, HEART)
        })
    {
        return false;
    }
    // A lone Exodia limb is not evidence of a supported draw engine. Any
    // unfamiliar ordinary Normal Monster in our visible resources qualifies.
    let normal_support = ctx.obs.cards.iter().any(|card| {
        card.at.controller == ctx.me
            && (card.at.location == Location::Hand
                || card.at.location == Location::MonsterZone && card.position.face_up)
            && card.code.is_some_and(|code| {
                exodia_piece(ctx.canonical(code)).is_none()
                    && ctx.data(code).is(types::MONSTER)
                    && ctx.data(code).is(types::NORMAL)
                    && !ctx.data(code).is(types::TOKEN)
            })
    });
    let offered = |predicate: &dyn Fn(u32) -> bool| {
        t.choices().any(|(_, choice)| {
            matches!(
                choice.kind,
                crate::model::ChoiceKind::Cards | crate::model::ChoiceKind::Toggle
            ) && choice.card.iter().chain(&choice.members).any(|member| {
                member.at.controller == ctx.me
                    && member.at.location == Location::Deck
                    && !member.required
                    && member
                        .code
                        .is_some_and(|code| predicate(ctx.canonical(code)))
            })
        })
    };
    normal_support
        && offered(&|code| code == HEART)
        && offered(&|code| exodia_piece(code).is_some() && copies_in_hand(&ctx, code) == 0)
}

pub(super) fn select(t: &Turn, member: &Member) -> Option<f64> {
    if member.at.controller != t.ctx.me {
        return None;
    }
    let code = t.ctx.canonical(member.code?);
    let hint = t.decision.hint;
    if code == HEART && member.at.location == Location::Deck && first_heart_from_duality(t) {
        // The guard is a single-card choice: express ordering directly without
        // changing Heart's keep value, evaluator weights, or other searches.
        return Some(f64::MAX);
    }
    if PIECES.contains(&code) {
        let copies = unselected_hand_copies(t, code);
        if hint == Hint::AddToHand
            || hint == Hint::ReturnToHand && member.at.location != Location::Hand
        {
            return Some(if copies == 0 {
                50_000.0 + distinct_pieces(&t.ctx) as f64 * 5000.0
            } else {
                1200.0
            });
        }
        if t.memory
            .last_activated
            .map(|source| t.ctx.canonical(source))
            == Some(GOLD_SARCOPHAGUS)
            && hint == Hint::Banish
            && member.at.location == Location::Deck
        {
            return Some(if copies == 0 { 14_000.0 } else { 0.0 });
        }
        if hint.is_cost() || hint == Hint::Destroy {
            return Some(if member.at.location == Location::Hand && copies <= 1 {
                -80_000.0
            } else if copies > 1 {
                -1200.0
            } else {
                -12_000.0
            });
        }
        if matches!(hint, Hint::Summon | Hint::SpecialSummon | Hint::Set)
            && member.at.location == Location::Hand
            && copies <= 1
        {
            return Some(-80_000.0);
        }
    }
    if code == BOARD || LETTERS.contains(&code) {
        if member.at.location.is_field()
            && t.ctx
                .card(member.at)
                .is_some_and(|card| card.position.face_up)
            && (hint.is_cost() || hint == Hint::Destroy)
        {
            // Any one piece leaving removes the rest of the displayed set.
            return Some(-80_000.0);
        }
        if t.memory
            .last_activated
            .map(|source| t.ctx.canonical(source))
            == Some(GOLD_SARCOPHAGUS)
            && hint == Hint::Banish
            && member.at.location == Location::Deck
            && code == BOARD
        {
            return Some(if board_visible(&t.ctx) { 0.0 } else { 20_000.0 });
        }
        if hint == Hint::AddToHand {
            return Some(if code == BOARD && !board_visible(&t.ctx) {
                16_000.0
            } else if LETTERS.contains(&code) {
                700.0
            } else {
                500.0
            });
        }
        if hint.is_cost() && member.at.location == Location::Hand {
            let unique_needed = unselected_hand_copies(t, code) <= 1
                && board_visible(&t.ctx)
                && !t.ctx.face_up_on_field(t.ctx.me, code);
            return Some(if unique_needed { -14_000.0 } else { -500.0 });
        }
    }
    None
}

pub(super) fn allow_staple(t: &Turn, code: u32) -> bool {
    let code = t.ctx.canonical(code);
    // Shared clearing counts our set cards and attack locks, but a displayed
    // Board is neither. Returning it also loses its accumulated messages.
    if code == GIANT_TRUNADE && board_active(&t.ctx) {
        return false;
    }
    if board_visible(&t.ctx)
        && permanent_backrow(&t.ctx, code)
        && t.choices().any(|(_, choice)| {
            choice.kind == crate::model::ChoiceKind::Activate
                && choice.code().map(|raw| t.ctx.canonical(raw)) == Some(code)
                && !t.view(choice).is_some_and(|card| {
                    card.at.location == Location::SpellTrapZone && card.position.face_up
                })
        })
    {
        return false;
    }
    match code {
        RELOAD if distinct_pieces(&t.ctx) > 0 => reload_heart(t),
        CARD_DESTRUCTION if distinct_pieces(&t.ctx) > 0 => false,
        // It is easy to stop paying this card before the final message; other
        // permanent backrow cards cannot be removed voluntarily that way.
        PEACE if board_active(&t.ctx) => letters_on_field(&t.ctx) < 3,
        _ => true,
    }
}

pub(super) fn allow_activation(t: &Turn, index: usize) -> Option<bool> {
    let choice = t.choice(index);
    let code = t.ctx.canonical(choice.code()?);
    let at = choice.at()?;
    if at.controller != t.ctx.me {
        return Some(false);
    }
    if !allow_staple(t, code) {
        return Some(false);
    }
    if LETTERS.contains(&code) {
        return Some(false);
    }
    if code == BOARD {
        let already_active = t.ctx.card(at).is_some_and(|card| card.position.face_up);
        return Some(already_active || !board_active(&t.ctx));
    }
    if code == HEART {
        // Face-up Heart has a real draw trigger; activating another copy from
        // hand is useful, but cannot permanently occupy a Board message slot.
        let already_active = at.location == Location::SpellTrapZone
            && t.view(choice).is_some_and(|card| card.position.face_up);
        if !already_active && board_visible(&t.ctx) {
            return Some(false);
        }
        return Some(true);
    }
    if board_visible(&t.ctx)
        && permanent_backrow(&t.ctx, code)
        && !t.view(choice).is_some_and(|card| {
            card.at.location == Location::SpellTrapZone && card.position.face_up
        })
    {
        // Reserving only the next slot still eventually deadlocks the final
        // message. Do not install a permanent occupant of any required slot.
        return Some(false);
    }
    None
}

pub(super) fn main_phase(t: &mut Turn) -> Option<usize> {
    for code in [BOARD, HEART] {
        if let Some(index) = t
            .activate(code)
            .filter(|&index| allow_activation(t, index) != Some(false))
        {
            // The offered Board action can be its already engine-gated End
            // Phase trigger; Heart's draw trigger is handled by chain below.
            let from_hand = t
                .choice(index)
                .at()
                .is_some_and(|at| at.location == Location::Hand);
            let face_down = t
                .view(t.choice(index))
                .is_none_or(|card| !card.position.face_up);
            if from_hand || face_down {
                return t.pick(index);
            }
        }
    }
    None
}

pub(super) fn chain(t: &Turn, index: usize) -> Option<Response> {
    let choice = t.choice(index);
    let code = t.ctx.canonical(choice.code()?);
    match code {
        HEART => Some(if safe_heart_draw(t) {
            Response::new(150.0)
        } else {
            Response::no()
        }),
        RELOAD if hearts(&t.ctx) > 0 || distinct_pieces(&t.ctx) > 0 => Some(if reload_heart(t) {
            Response::new(90.0)
        } else {
            Response::no()
        }),
        CARD_DESTRUCTION if distinct_pieces(&t.ctx) > 0 => Some(Response::no()),
        BOARD => Some(if allow_activation(t, index) != Some(false) {
            Response::new(130.0)
        } else {
            Response::no()
        }),
        _ => None,
    }
}

pub(super) fn set_spell_trap(t: &Turn, code: u32) -> Option<bool> {
    let code = t.ctx.canonical(code);
    if LETTERS.contains(&code) {
        return Some(false);
    }
    if code == BOARD {
        return Some(t.ctx.count_in(t.ctx.me, Location::SpellTrapZone, BOARD) == 0);
    }
    if !board_visible(&t.ctx) {
        return None;
    }
    if permanent_backrow(&t.ctx, code) {
        return Some(false);
    }
    // Reserve a space for the next automatic message. Before the Board itself
    // is set, reserve its slot too. A field card uses a separate sixth zone.
    if t.ctx.data(code).is(types::FIELD) {
        return None;
    }
    // This is a space veto, not a reason to set an otherwise unhelpful Spell.
    // Preserve the normal trap/Quick-Play setting policy while space remains.
    (used_spell_zones(&t.ctx) >= if board_active(&t.ctx) { 4 } else { 3 }).then_some(false)
}

pub(super) fn yes_no(t: &Turn) -> Option<bool> {
    if t.decision.subject.map(|code| t.ctx.canonical(code)) == Some(HEART) {
        return Some(safe_heart_draw(t));
    }
    let peace_maintenance = t.decision.subject.map(|code| t.ctx.canonical(code)) == Some(PEACE)
        || t.choices()
            .any(|(_, choice)| choice.description == (PEACE as u64) << 20);
    if peace_maintenance && board_active(&t.ctx) && letters_on_field(&t.ctx) >= 3 {
        return Some(false);
    }
    None
}

pub(super) fn allow_repeated_chain(t: &Turn, index: usize) -> bool {
    t.choice(index).code().map(|code| t.ctx.canonical(code)) == Some(HEART)
        && matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. })
        && safe_heart_draw(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, Memory, Policy};
    use crate::cards::{CardData, MemoryCards};
    use crate::model::{CardRef, CardView, ChainLink, ChoiceKind, Decision, Observation, Position};

    fn observation() -> Observation {
        Observation {
            me: 0,
            turn: 1,
            turn_player: Some(0),
            phase: Some(Phase::Main1),
            life_points: [8000; 2],
            cards: vec![],
            pile_sizes: vec![(0, Location::Deck, 20)],
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
    fn card(code: u32, location: Location, sequence: u32) -> CardView {
        CardView {
            at: CardRef {
                controller: 0,
                location,
                sequence,
            },
            code: Some(code),
            position: Position::FACE_UP_ATTACK,
            attack: 0,
            defense: 0,
            level: 1,
            can_attack: false,
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
    fn decision(hint: Hint) -> Decision {
        Decision {
            kind: DecisionKind::SelectCards,
            hint,
            minimum: 1,
            maximum: 1,
            selected: vec![],
            subject: None,
            choices: vec![],
        }
    }
    fn with_turn<R>(
        observation: &Observation,
        decision: &Decision,
        f: impl FnOnce(&mut Turn) -> R,
    ) -> R {
        let db = MemoryCards::default();
        let mut memory = Memory::default();
        f(&mut Turn {
            ctx: Ctx::new(observation, &db),
            decision,
            memory: &mut memory,
        })
    }

    #[test]
    fn pieces_are_preserved_but_duplicates_and_unrelated_cards_are_not_locked() {
        let mut obs = observation();
        obs.cards.push(card(PIECES[0], Location::Hand, 0));
        obs.cards.push(card(900_400_001, Location::Hand, 1));
        let prompt = decision(Hint::Discard);
        with_turn(&obs, &prompt, |t| {
            assert_eq!(select(t, &member(&obs.cards[0])), Some(-80_000.0));
            assert_eq!(select(t, &member(&obs.cards[1])), None);
        });
        obs.cards.push(card(PIECES[0], Location::Hand, 2));
        with_turn(&obs, &prompt, |t| {
            assert_eq!(select(t, &member(&obs.cards[0])), Some(-1200.0))
        });
        let missing = card(PIECES[1], Location::Graveyard, 0);
        let gain = decision(Hint::AddToHand);
        with_turn(&obs, &gain, |t| {
            assert!(
                select(t, &member(&missing)).unwrap() > select(t, &member(&obs.cards[0])).unwrap()
            )
        });
    }

    #[test]
    fn sequential_discard_preserves_the_final_copy_after_spending_a_duplicate() {
        let mut obs = observation();
        obs.cards = vec![
            card(PIECES[0], Location::Hand, 0),
            card(PIECES[0], Location::Hand, 1),
            card(HEART, Location::Hand, 2),
        ];
        let db = std::sync::Arc::new(MemoryCards(
            [
                (
                    PIECES[0],
                    CardData {
                        code: PIECES[0],
                        kind: types::MONSTER | types::EFFECT,
                        ..CardData::default()
                    },
                ),
                (
                    HEART,
                    CardData {
                        code: HEART,
                        kind: types::SPELL | types::CONTINUOUS,
                        ..CardData::default()
                    },
                ),
            ]
            .into(),
        ));
        let mut pilot = Agent::seeded(super::super::Generic, db, 1);
        let mut prompt = decision(Hint::Discard);
        prompt.minimum = 2;
        prompt.maximum = 2;
        prompt.choices = obs
            .cards
            .iter()
            .map(|card| Choice {
                kind: ChoiceKind::Toggle,
                card: Some(member(card)),
                members: vec![],
                description: 0,
                place: None,
            })
            .collect();
        let first = pilot.choose(&obs, &prompt);
        let picked = prompt.choices.remove(first).card.unwrap();
        assert_eq!(picked.code, Some(PIECES[0]));
        prompt.selected.push(picked.at);
        let second = pilot.choose(&obs, &prompt);
        assert_eq!(prompt.choices[second].code(), Some(HEART));
    }

    #[test]
    fn generic_does_not_clear_its_own_displayed_board_to_open_an_attack() {
        let wipe = GIANT_TRUNADE;
        for displayed_board in [false, true] {
            let mut obs = observation();
            let source = card(wipe, Location::Hand, 0);
            let mut attacker = card(900_400_020, Location::MonsterZone, 0);
            attacker.attack = 1900;
            attacker.can_attack = true;
            obs.cards = vec![source.clone(), attacker];
            for sequence in 0..2 {
                let mut backrow = card(900_400_021, Location::SpellTrapZone, sequence);
                backrow.at.controller = 1;
                backrow.code = None;
                backrow.position = Position::FACE_DOWN_DEFENSE;
                obs.cards.push(backrow);
            }
            if displayed_board {
                obs.cards.push(card(BOARD, Location::SpellTrapZone, 0));
                obs.cards.push(card(LETTERS[0], Location::SpellTrapZone, 1));
            }
            let db = std::sync::Arc::new(MemoryCards(
                [
                    (
                        wipe,
                        CardData {
                            code: wipe,
                            kind: types::SPELL,
                            ..CardData::default()
                        },
                    ),
                    (
                        900_400_020,
                        CardData {
                            code: 900_400_020,
                            kind: types::MONSTER,
                            attack: 1900,
                            level: 4,
                            ..CardData::default()
                        },
                    ),
                ]
                .into(),
            ));
            let mut pilot = Agent::seeded(super::super::Generic, db, 1);
            let mut prompt = decision(Hint::None);
            prompt.kind = DecisionKind::Idle;
            prompt.choices = vec![
                Choice {
                    kind: ChoiceKind::Activate,
                    card: Some(member(&source)),
                    members: vec![],
                    description: 0,
                    place: None,
                },
                Choice {
                    kind: ChoiceKind::EnterBattle,
                    card: None,
                    members: vec![],
                    description: 0,
                    place: None,
                },
                Choice {
                    kind: ChoiceKind::EndTurn,
                    card: None,
                    members: vec![],
                    description: 0,
                    place: None,
                },
            ];
            let chosen = pilot.choose(&obs, &prompt);
            if displayed_board {
                assert_ne!(chosen, 0, "{wipe}");
            } else {
                assert_eq!(chosen, 0, "{wipe}");
            }
        }
    }

    #[test]
    fn repeated_heart_draws_require_deck_space_for_every_pending_draw() {
        let mut obs = observation();
        obs.phase = Some(Phase::Draw);
        obs.pile_sizes = vec![(0, Location::Deck, 1)];
        let source = card(HEART, Location::SpellTrapZone, 0);
        let mut prompt = decision(Hint::None);
        prompt.kind = DecisionKind::Chain {
            forced: false,
            triggers: true,
        };
        prompt.choices.push(Choice {
            kind: ChoiceKind::Activate,
            card: Some(member(&source)),
            members: vec![],
            description: 0,
            place: None,
        });
        with_turn(&obs, &prompt, |t| {
            assert!(allow_repeated_chain(t, 0));
            assert!(chain(t, 0).unwrap().score > 0.0);
        });
        obs.chain.push(ChainLink {
            code: HEART,
            controller: 0,
            source: source.at,
            targets: vec![],
        });
        with_turn(&obs, &prompt, |t| {
            assert!(!allow_repeated_chain(t, 0));
            assert_eq!(chain(t, 0).unwrap().score, 0.0);
        });
        obs.chain.clear();
        obs.phase = Some(Phase::Main1);
        with_turn(&obs, &prompt, |t| assert!(!allow_repeated_chain(t, 0)));
    }

    #[test]
    fn board_space_is_reserved_only_when_the_card_is_visible_and_excludes_field_zone() {
        let mut obs = observation();
        let prompt = decision(Hint::None);
        with_turn(&obs, &prompt, |t| {
            assert_eq!(set_spell_trap(t, 900_400_002), None)
        });
        obs.cards.push(card(BOARD, Location::SpellTrapZone, 0));
        obs.cards
            .push(card(900_400_003, Location::SpellTrapZone, 5));
        for sequence in 1..=2 {
            obs.cards.push(card(
                LETTERS[sequence as usize - 1],
                Location::SpellTrapZone,
                sequence,
            ));
        }
        with_turn(&obs, &prompt, |t| {
            assert_eq!(set_spell_trap(t, 900_400_002), None)
        });
        obs.cards.push(card(LETTERS[2], Location::SpellTrapZone, 3));
        with_turn(&obs, &prompt, |t| {
            assert_eq!(set_spell_trap(t, 900_400_002), Some(false));
            assert_eq!(set_spell_trap(t, BOARD), Some(false));
            assert_eq!(set_spell_trap(t, LETTERS[3]), Some(false));
        });
    }

    #[test]
    fn board_does_not_install_a_new_permanent_card_even_with_several_free_slots() {
        const UNFAMILIAR_CONTINUOUS: u32 = 900_400_010;
        const UNFAMILIAR_FIELD: u32 = 900_400_011;
        let mut obs = observation();
        obs.cards.push(card(BOARD, Location::SpellTrapZone, 0));
        let permanent = card(UNFAMILIAR_CONTINUOUS, Location::Hand, 0);
        let mut prompt = decision(Hint::None);
        prompt.choices.push(Choice {
            kind: ChoiceKind::Activate,
            card: Some(member(&permanent)),
            members: vec![],
            description: 0,
            place: None,
        });
        let db = MemoryCards(
            [
                (
                    UNFAMILIAR_CONTINUOUS,
                    CardData {
                        code: UNFAMILIAR_CONTINUOUS,
                        kind: types::SPELL | types::CONTINUOUS,
                        ..CardData::default()
                    },
                ),
                (
                    UNFAMILIAR_FIELD,
                    CardData {
                        code: UNFAMILIAR_FIELD,
                        kind: types::SPELL | types::FIELD,
                        ..CardData::default()
                    },
                ),
            ]
            .into(),
        );
        let mut memory = Memory::default();
        let turn = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        };
        assert_eq!(used_spell_zones(&turn.ctx), 1);
        assert_eq!(allow_activation(&turn, 0), Some(false));
        assert!(!allow_staple(&turn, UNFAMILIAR_CONTINUOUS));
        assert_eq!(set_spell_trap(&turn, UNFAMILIAR_CONTINUOUS), Some(false));
        assert_eq!(set_spell_trap(&turn, UNFAMILIAR_FIELD), None);
    }

    #[test]
    fn reload_does_not_shuffle_away_a_nearly_complete_hand_in_an_unfamiliar_mix() {
        let mut obs = observation();
        obs.phase = Some(Phase::Draw);
        obs.cards.push(card(HEART, Location::SpellTrapZone, 0));
        for (sequence, code) in PIECES.iter().take(4).enumerate() {
            obs.cards.push(card(*code, Location::Hand, sequence as u32));
        }
        let db = MemoryCards(
            PIECES
                .iter()
                .map(|&code| {
                    (
                        code,
                        CardData {
                            code,
                            kind: types::MONSTER | types::NORMAL,
                            ..CardData::default()
                        },
                    )
                })
                .collect(),
        );
        let prompt = decision(Hint::None);
        let mut memory = Memory::default();
        let turn = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        };
        assert!(!allow_staple(&turn, RELOAD));
        assert!(!allow_staple(&turn, CARD_DESTRUCTION));
        assert!(allow_staple(&turn, 900_400_004));
    }

    const ORDINARY_NORMAL: u32 = 900_400_030;
    const ORDINARY_BACKROW: u32 = 900_400_031;

    fn duality_fixture(me: u8) -> (Observation, Decision, MemoryCards) {
        let mut obs = observation();
        obs.me = me;
        obs.turn_player = Some(me);
        obs.pile_sizes = vec![(me, Location::Deck, 30)];
        obs.cards = vec![
            card(PIECES[1], Location::Hand, 0),
            card(ORDINARY_NORMAL, Location::Hand, 1),
            card(DUALITY, Location::SpellTrapZone, 1),
            card(ORDINARY_BACKROW, Location::SpellTrapZone, 2),
            card(ORDINARY_BACKROW, Location::SpellTrapZone, 3),
        ];
        for card in &mut obs.cards {
            card.at.controller = me;
        }
        obs.chain.push(ChainLink {
            code: DUALITY,
            controller: me,
            source: obs.cards[2].at,
            targets: vec![],
        });
        let mut prompt = decision(Hint::AddToHand);
        prompt.choices = [PIECES[4], HEART, ORDINARY_NORMAL]
            .into_iter()
            .enumerate()
            .map(|(sequence, code)| {
                let mut candidate = card(code, Location::Deck, sequence as u32);
                candidate.at.controller = me;
                Choice {
                    kind: ChoiceKind::Cards,
                    card: None,
                    members: vec![member(&candidate)],
                    description: 0,
                    place: None,
                }
            })
            .collect();
        let mut db = MemoryCards::default();
        for code in PIECES.into_iter().chain([HEART, DUALITY, ORDINARY_NORMAL]) {
            db.0.insert(
                code,
                CardData {
                    code,
                    kind: if code == HEART {
                        types::SPELL | types::CONTINUOUS
                    } else if code == DUALITY {
                        types::SPELL
                    } else if code == PIECES[0] {
                        types::MONSTER | types::EFFECT
                    } else {
                        types::MONSTER | types::NORMAL
                    },
                    attack: if code == ORDINARY_NORMAL { 1000 } else { 0 },
                    defense: if code == ORDINARY_NORMAL { 2000 } else { 0 },
                    level: 4,
                    ..CardData::default()
                },
            );
        }
        (obs, prompt, db)
    }

    fn first_heart_enabled(obs: &Observation, prompt: &Decision, db: &MemoryCards) -> bool {
        let mut memory = Memory::default();
        memory.last_activated = Some(DUALITY);
        first_heart_from_duality(&Turn {
            ctx: Ctx::new(obs, db),
            decision: prompt,
            memory: &mut memory,
        })
    }

    fn duality_pick(obs: &Observation, prompt: &Decision, db: &MemoryCards) -> usize {
        let mut pilot = Agent::seeded(
            super::super::Generic,
            std::sync::Arc::new(MemoryCards(db.0.clone())),
            1,
        );
        let mut activation = decision(Hint::None);
        activation.kind = DecisionKind::Idle;
        activation.choices.push(Choice {
            kind: ChoiceKind::Activate,
            card: Some(member(
                obs.cards
                    .iter()
                    .find(|card| card.code == Some(DUALITY))
                    .unwrap(),
            )),
            members: vec![],
            description: (DUALITY as u64) << 20,
            place: None,
        });
        assert!(pilot.commit_choice(obs, &activation, 0));
        pilot.choose(obs, prompt)
    }

    #[test]
    fn duality_establishes_first_heart_with_unfamiliar_normal_support_on_either_seat() {
        // Frozen v17 seed1326585873 seat1 trace108 offered Left Leg, Heart,
        // and Charcoal Inpachi while only Right Arm was held and no Heart
        // was available. Use an unfamiliar Normal to avoid a deck recipe.
        for me in [0, 1] {
            for held_piece in [false, true] {
                let (mut obs, mut prompt, db) = duality_fixture(me);
                if !held_piece {
                    obs.cards.retain(|card| card.code != Some(PIECES[1]));
                }
                assert!(first_heart_enabled(&obs, &prompt, &db));
                assert_eq!(duality_pick(&obs, &prompt, &db), 1);
                // The native sequential path presents a Toggle for each
                // legal candidate rather than a complete Cards answer.
                for choice in &mut prompt.choices {
                    choice.kind = ChoiceKind::Toggle;
                    choice.card = choice.members.first().copied();
                }
                assert_eq!(duality_pick(&obs, &prompt, &db), 1);
                assert_eq!(value(&Ctx::new(&obs, &db), HEART), Some(7000));
            }
        }
    }

    #[test]
    fn duality_completion_uses_distinct_names_and_keeps_the_fifth_piece_first() {
        for me in [0, 1] {
            let (mut obs, prompt, db) = duality_fixture(me);
            obs.cards.retain(|card| card.at.location != Location::Hand);
            for (sequence, code) in [
                PIECES[0],
                PIECES[1],
                PIECES[2],
                PIECES[3],
                PIECES[0],
                ORDINARY_NORMAL,
            ]
            .into_iter()
            .enumerate()
            {
                let mut hand = card(code, Location::Hand, sequence as u32);
                hand.at.controller = me;
                obs.cards.push(hand);
            }
            assert!(!first_heart_enabled(&obs, &prompt, &db));
            assert_eq!(duality_pick(&obs, &prompt, &db), 0);
            // Four cards containing only three names cannot complete Exodia.
            obs.cards.retain(|card| card.code != Some(PIECES[3]));
            assert!(first_heart_enabled(&obs, &prompt, &db));
            assert_eq!(duality_pick(&obs, &prompt, &db), 1);
        }
    }

    #[test]
    fn duality_first_heart_keeps_existing_engine_behavior() {
        for (location, position) in [
            (Location::Hand, Position::FACE_DOWN_ATTACK),
            (Location::SpellTrapZone, Position::FACE_UP_ATTACK),
            (Location::SpellTrapZone, Position::FACE_DOWN_DEFENSE),
        ] {
            let (mut obs, prompt, db) = duality_fixture(0);
            let mut heart = card(HEART, location, 4);
            heart.position = position;
            obs.cards.push(heart);
            assert!(!first_heart_enabled(&obs, &prompt, &db));
            assert_eq!(duality_pick(&obs, &prompt, &db), 0);
        }
    }

    #[test]
    fn duality_first_heart_requires_visible_support_and_a_place_to_install_it() {
        let (base, prompt, db) = duality_fixture(0);
        let mut obs = base.clone();
        obs.cards.retain(|card| card.code != Some(ORDINARY_NORMAL));
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        let mut enemy = card(ORDINARY_NORMAL, Location::MonsterZone, 0);
        enemy.at.controller = 1;
        obs.cards.push(enemy);
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        let mut hidden = card(ORDINARY_NORMAL, Location::MonsterZone, 0);
        hidden.position = Position::FACE_DOWN_DEFENSE;
        obs.cards.push(hidden);
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        obs.cards.last_mut().unwrap().position = Position::FACE_UP_DEFENSE;
        assert!(first_heart_enabled(&obs, &prompt, &db));

        let mut obs = base.clone();
        obs.cards
            .push(card(ORDINARY_BACKROW, Location::SpellTrapZone, 0));
        obs.cards
            .push(card(ORDINARY_BACKROW, Location::SpellTrapZone, 4));
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        obs.cards.last_mut().unwrap().at.sequence = 5;
        assert!(
            first_heart_enabled(&obs, &prompt, &db),
            "Field Zone is separate"
        );
        let mut obs = base.clone();
        obs.cards.push(card(BOARD, Location::Hand, 4));
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        let mut obs = base.clone();
        obs.pile_sizes = vec![(0, Location::Deck, 1)];
        assert!(!first_heart_enabled(&obs, &prompt, &db));
        let mut obs = base;
        let mut negator = card(33198837, Location::MonsterZone, 0);
        negator.at.controller = 1;
        obs.cards.push(negator);
        assert!(
            !first_heart_enabled(&obs, &prompt, &db),
            "shared known Spell negation"
        );
    }

    #[test]
    fn duality_first_heart_does_not_change_other_searches_or_unoffered_choices() {
        let (obs, prompt, db) = duality_fixture(0);
        for hint in [Hint::ToGraveyard, Hint::ReturnToHand, Hint::Discard] {
            let mut wrong = prompt.clone();
            wrong.hint = hint;
            assert!(!first_heart_enabled(&obs, &wrong, &db));
        }
        for kind in [DecisionKind::SelectSum, DecisionKind::SelectToggle] {
            let mut wrong = prompt.clone();
            wrong.kind = kind;
            assert!(!first_heart_enabled(&obs, &wrong, &db));
        }
        let mut wrong = prompt.clone();
        wrong.maximum = 2;
        assert!(!first_heart_enabled(&obs, &wrong, &db));
        let mut wrong = prompt.clone();
        wrong.choices.remove(1);
        assert!(!first_heart_enabled(&obs, &wrong, &db));
        let mut wrong = prompt.clone();
        wrong.choices[1].members[0].at.location = Location::Graveyard;
        assert!(!first_heart_enabled(&obs, &wrong, &db));
        let mut wrong = prompt.clone();
        wrong.choices[0].members[0].code = Some(PIECES[1]);
        assert!(!first_heart_enabled(&obs, &wrong, &db));
        let mut wrong = obs.clone();
        wrong.chain_known = false;
        assert!(!first_heart_enabled(&wrong, &prompt, &db));
        let mut wrong = obs.clone();
        wrong.chain[0].controller = 1;
        assert!(!first_heart_enabled(&wrong, &prompt, &db));
        wrong.chain.clear();
        assert!(!first_heart_enabled(&wrong, &prompt, &db));
        let mut memory = Memory::default();
        memory.last_activated = Some(RELOAD);
        assert!(!first_heart_from_duality(&Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        }));
    }

    #[test]
    fn duality_first_heart_is_invariant_to_opponent_hidden_card_identity() {
        for me in [0, 1] {
            let (mut obs, prompt, db) = duality_fixture(me);
            let mut hidden = card(HEART, Location::Hand, 0);
            hidden.at.controller = 1 - me;
            hidden.position = Position::FACE_DOWN_ATTACK;
            obs.cards.push(hidden);
            for code in [None, Some(HEART), Some(PIECES[0]), Some(ORDINARY_NORMAL)] {
                obs.cards.last_mut().unwrap().code = code;
                assert!(first_heart_enabled(&obs, &prompt, &db));
                assert_eq!(duality_pick(&obs, &prompt, &db), 1);
            }
        }
    }
}
