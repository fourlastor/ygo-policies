//! Necessary effect conditions and origin-sensitive resource payoffs.
//!
//! Facts are taken from `vendor/CardScripts/official/c<code>.lua` for cards
//! in the current supplied main/extra pool. They are attached to individual
//! effects, not to decks. Satisfying a recorded condition does not establish
//! legality: targets, events, summon restrictions and future resolution are
//! still the engine's responsibility.

use crate::agent::Turn;
use crate::cards::{effect_events as event, effect_locations as range,
    effect_types as kind, EffectDescriptor};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

const ARCHFIEND: u32 = 99177923;
const NECROMANCER: u32 = 56209279;
const BEETLE: u32 = 49080532;
const MIRAGE: u32 = 86197239;
const LAUNCHER: u32 = 66957584;
const DOOM_DRAGON: u32 = 72896720;
const AVENGER: u32 = 85475641;
const FORCE: u32 = 18712704;
const GUARDIAN: u32 = 51566770;
const RAGIN: u32 = 47395382;
const CERBURREL: u32 = 82888408;
const GANASHIA: u32 = 18282103;
const LURRIE: u32 = 97651498;
const KRUS: u32 = 19439119;
const CATSITH: u32 = 56399890;
const CHAWA: u32 = 29905795;
const NOZOOCHEE: u32 = 55277252;
const INFERNITY_SET: u16 = 0xb;
const FABLED_SET: u16 = 0x35;
const BATTLE_DESTROYED: u32 = 1140;
const BE_BATTLE_TARGET: u32 = 1131;

/// At least one explicit rule is described for each entry. This is not a
/// claim to cover every effect or interaction of these cards.
pub const SUPPORTED_CODES: &[u32] = &[
    ARCHFIEND, NECROMANCER, BEETLE, MIRAGE, LAUNCHER, DOOM_DRAGON,
    AVENGER, FORCE, GUARDIAN, RAGIN, CERBURREL, GANASHIA, LURRIE,
    KRUS, CATSITH, CHAWA, NOZOOCHEE,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandRequirement {
    Empty,
    Below(u32),
}

impl HandRequirement {
    pub fn allows(self, count: u32) -> bool {
        match self { Self::Empty => count == 0, Self::Below(limit) => count < limit }
    }
}

const fn description(code: u32, index: u64) -> u64 { ((code as u64) << 20) | index }

/// `code` is the canonical visible passcode. Match the complete effect slot
/// so, for example, Launcher's unrestricted send effect remains available.
pub fn hand_requirement(code: u32, effect: &EffectDescriptor) -> Option<HandRequirement> {
    let (index, event_code, effect_kind, location, requirement) = match code {
        NECROMANCER => (1, 0, kind::IGNITION, range::MONSTER, HandRequirement::Empty),
        BEETLE | MIRAGE | DOOM_DRAGON => (0, 0, kind::IGNITION, range::MONSTER, HandRequirement::Empty),
        LAUNCHER => (1, 0, kind::IGNITION, range::SPELL_TRAP, HandRequirement::Empty),
        AVENGER => (0, BATTLE_DESTROYED, kind::TRIGGER, range::GRAVEYARD, HandRequirement::Empty),
        ARCHFIEND => (1, event::SPECIAL_SUMMON_SUCCESS, kind::TRIGGER, 0, HandRequirement::Empty),
        FORCE => {
            return (effect.description == 0 && effect.event == BE_BATTLE_TARGET
                && effect.effect_type & kind::ACTIVATE != 0).then_some(HandRequirement::Empty);
        }
        // This is only a necessary hand condition. The script also requires
        // that this particular summon was a Synchro Summon, not revival.
        RAGIN => (0, event::SPECIAL_SUMMON_SUCCESS, kind::TRIGGER, 0, HandRequirement::Below(2)),
        _ => return None,
    };
    (effect.description == description(code, index) && effect.event == event_code
        && effect.effect_type & effect_kind != 0
        && if location == 0 { effect.range == 0 } else { range::normalize(effect.range) & location != 0 })
        .then_some(requirement)
}

/// Unknown conditions remain unknown. Archfiend's draw trigger deliberately
/// has no current-hand requirement: it tests whether the hand was empty
/// before the draw, and is offered while the freshly drawn card is in hand.
pub fn hand_condition_allows(code: u32, effect: &EffectDescriptor, hand_size: u32) -> bool {
    hand_requirement(code, effect).is_none_or(|requirement| requirement.allows(hand_size))
}

/// Immediate payoff of a source-backed native Extra Deck summon procedure.
/// These facts do not describe revivals or a monster already on the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraSummonPayoff {
    DrawToHandSize { limit: u32 },
}

pub fn extra_summon_payoff(code: u32) -> Option<ExtraSummonPayoff> {
    match code {
        // c47395382 registers only its Synchro procedure. Its optional
        // summon trigger draws exactly enough cards to reach two in hand.
        RAGIN => Some(ExtraSummonPayoff::DrawToHandSize { limit: 2 }),
        _ => None,
    }
}

/// Prospective draw count for the exact offered native procedure. This is a
/// public-state prior: the engine still resolves responses and draw legality.
pub(super) fn extra_summon_draws(t: &Turn, choice: &Choice) -> Option<u32> {
    let ctx = t.ctx;
    if choice.kind != ChoiceKind::SpecialSummon || choice.description != 0
        || choice.at().is_none_or(|at| at.controller != ctx.me || at.location != Location::Extra)
    {
        return None;
    }
    let code = ctx.canonical(choice.code()?);
    let ExtraSummonPayoff::DrawToHandSize { limit } = extra_summon_payoff(code)?;
    let count = limit.saturating_sub(ctx.hand_size(ctx.me));
    Some(if ctx.effects_drained() || ctx.wasted(code) || ctx.deck_size(ctx.me) < count { 0 } else { count })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrivalRequirement {
    /// Both discard costs and discard effects qualify; ordinary sends,
    /// material use and tributes do not. The card must actually reach the GY.
    DiscardedFromHand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscardPayoff { SummonSelf, ReviveSmallFabled, DestroyFaceUp }

fn discard_payoff(code: u32) -> Option<DiscardPayoff> {
    Some(match code {
        CERBURREL | GANASHIA | LURRIE => DiscardPayoff::SummonSelf,
        KRUS => DiscardPayoff::ReviveSmallFabled,
        CATSITH => DiscardPayoff::DestroyFaceUp,
        _ => return None,
    })
}

/// Known arrival conditions must be removed from unconditional metadata GY
/// option value. Their transaction-specific credit comes from arrival_value.
pub fn arrival_requirement(code: u32, effect: &EffectDescriptor) -> Option<ArrivalRequirement> {
    (discard_payoff(code).is_some() && effect.description == description(code, 0)
        && effect.event == event::TO_GRAVE && effect.effect_type & kind::TRIGGER != 0)
        .then_some(ArrivalRequirement::DiscardedFromHand)
}

/// A context-sensitive prior for the five supported arrival-only cards.
/// These cards have no separate activated GY action to preserve here. `None`
/// leaves other cards' metadata fallback intact; `Some(0)` rejects a false
/// trigger credit, not the legal selection itself.
pub(super) fn arrival_value(t: &Turn, member: &Member) -> Option<f64> {
    let payoff = discard_payoff(t.ctx.canonical(member.code?))?;
    if member.at.controller != t.ctx.me || member.at.location != Location::Hand
        || t.decision.hint != Hint::Discard || t.ctx.monsters_banished()
    {
        return Some(0.0);
    }
    let ctx = t.ctx;
    // Chawa/Nozoochee discard during resolution and summon themselves before
    // the discarded card's trigger resolves. Reserve that occupied zone.
    let pending_body = t.memory.last_activated.is_some_and(|code| {
        matches!(ctx.canonical(code), CHAWA | NOZOOCHEE)
            && ctx.in_hand(ctx.canonical(code))
    });
    Some(discard_payoff_value(t, payoff, usize::from(pending_body)))
}

/// Shared estimate for an actual discard and a prospective hand outlet.
/// `reserved_zones` accounts for bodies deployed before the arrival trigger.
/// The caller establishes the origin/reason and that the card reaches the GY.
fn discard_payoff_value(t: &Turn, payoff: DiscardPayoff, reserved_zones: usize) -> f64 {
    let ctx = t.ctx;
    let space = ctx.free_monster_zones(ctx.me) > reserved_zones;
    match payoff {
        DiscardPayoff::SummonSelf => if space { 3000.0 } else { 0.0 },
        DiscardPayoff::ReviveSmallFabled => {
            // Another copy of Krus is legal: the script excludes the actual
            // discarded card, not every card with the same passcode.
            let target = ctx.graveyard(ctx.me).into_iter().any(|card| {
                let data = ctx.view_data(card);
                data.is_monster() && data.in_set(FABLED_SET) && data.level <= 4
            });
            if space && target { 3200.0 } else { 0.0 }
        }
        DiscardPayoff::DestroyFaceUp => {
            // Catsith's trigger is mandatory and can destroy our own card.
            // Do not pay for it using an enemy already scheduled to leave
            // through the current targeted transaction.
            let target = ctx.obs.cards.iter().any(|card| {
                card.at.controller == ctx.opp && card.at.location.is_field() && card.position.face_up
                    && ctx.reaches(card, CATSITH, true, true)
                    && !t.memory.intent.iter().any(|(at, _)| *at == card.at)
            });
            if target {
                3500.0
            } else {
                let own_loss = ctx.obs.cards.iter().filter(|card| {
                    card.at.controller == ctx.me && card.at.location.is_field() && card.position.face_up
                        && ctx.reaches(card, CATSITH, true, true)
                }).map(|card| ctx.threat(card).max(500)).min().unwrap_or(0);
                // The selection already pays for discarding Catsith. This
                // additional loss is the compulsory destruction afterwards.
                -(own_loss as f64)
            }
        }
    }
}

/// c29905795.spop and c55277252.op discard another Fabled monster from hand
/// as an effect, then summon the handler before the discarded card's trigger.
/// Advance an engine-offered hand ignition when a visible discard has a known
/// positive payoff. This uses no deck identity or hidden remaining inventory.
/// The caller retains activation vetoes and the normal action-loop guard.
pub(super) fn activate_before_normal(t: &Turn, choice: &Choice) -> bool {
    let ctx = t.ctx;
    let Some(code) = choice.code().map(|code| ctx.canonical(code)) else { return false; };
    if choice.kind != ChoiceKind::Activate || !matches!(code, CHAWA | NOZOOCHEE)
        || choice.description != description(code, 0)
        || choice.at().is_none_or(|at| at.controller != ctx.me || at.location != Location::Hand)
        || ctx.monsters_banished() || ctx.free_monster_zones(ctx.me) == 0
    {
        return false;
    }
    ctx.hand().into_iter().any(|fuel| {
        if Some(fuel.at) == choice.at() { return false; }
        let data = ctx.view_data(fuel);
        if !data.is_monster() || !data.in_set(FABLED_SET) { return false; }
        fuel.code.and_then(|code| discard_payoff(ctx.canonical(code)))
            .is_some_and(|payoff| discard_payoff_value(t, payoff, 1) > 0.0)
    })
}

fn infernity_revival_targets(ctx: &Ctx, exclude_necromancer: bool) -> usize {
    ctx.graveyard(ctx.me).into_iter().filter(|card| {
        let data = ctx.view_data(card);
        let code = card.code.map(|code| ctx.canonical(code));
        data.is_monster() && data.in_set(INFERNITY_SET) && code != Some(MIRAGE)
            && (!exclude_necromancer || code != Some(NECROMANCER))
    }).count()
}

fn live_empty_hand_payoff(ctx: &Ctx, card: &CardView) -> bool {
    let Some(code) = card.code.map(|code| ctx.canonical(code)) else { return false; };
    if card.at.controller != ctx.me { return false; }
    match (card.at.location, code) {
        (Location::MonsterZone, _) if !card.position.face_up || ctx.effects_drained() => false,
        (Location::MonsterZone, NECROMANCER) => infernity_revival_targets(ctx, true) > 0,
        (Location::MonsterZone, MIRAGE) => infernity_revival_targets(ctx, false) >= 2,
        // Unknown remaining deck identities are not inspected. This is an
        // ordering prior; the engine still checks that copies remain.
        (Location::MonsterZone, BEETLE) => ctx.deck_size(ctx.me) > 0,
        (Location::MonsterZone, GUARDIAN) => true,
        (Location::MonsterZone, DOOM_DRAGON) => ctx.monsters(ctx.opp).into_iter()
            .any(|enemy| ctx.reaches(enemy, code, true, true)),
        (Location::SpellTrapZone, LAUNCHER) => card.position.face_up && infernity_revival_targets(ctx, false) > 0,
        (Location::SpellTrapZone, FORCE) => infernity_revival_targets(ctx, false) > 0
            && ctx.monsters(ctx.me).into_iter().any(|body| body.position.face_up
                && ctx.view_data(body).in_set(INFERNITY_SET)),
        _ => false,
    }
}

/// Set a legal backrow card before a currently visible empty-hand payoff or
/// an offered Ragin Synchro Summon. The caller retains its normal setting
/// vetoes (for example, alternate-win pieces) and action-loop guard.
pub(super) fn set_before_payoff(t: &Turn, choice: &Choice) -> bool {
    if choice.kind != ChoiceKind::SetSpellTrap
        || choice.at().is_none_or(|at| at.controller != t.ctx.me || at.location != Location::Hand)
    {
        return false;
    }
    let ctx = t.ctx;
    if ctx.hand_size(ctx.me) == 0 { return false; }
    if ctx.obs.cards.iter().any(|card| live_empty_hand_payoff(&ctx, card)) { return true; }
    ctx.hand_size(ctx.me) >= 2 && !ctx.effects_drained()
        && t.choices().any(|(_, offered)| offered.kind == ChoiceKind::SpecialSummon
            && offered.code().is_some_and(|code| ctx.canonical(code) == RAGIN)
            && offered.at().is_some_and(|at| at.controller == ctx.me && at.location == Location::Extra))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{types, CardData, MemoryCards};
    use crate::model::{CardRef, Decision, DecisionKind, Observation, Phase, Position};

    fn effect(code: u32, index: u64, event: u32, effect_type: u16, range: u16) -> EffectDescriptor {
        EffectDescriptor { description: description(code, index), event, effect_type, range, ..EffectDescriptor::default() }
    }

    #[test]
    fn hand_requirements_match_effect_slots_without_gating_unrelated_effects() {
        let revive = effect(LAUNCHER, 1, 0, kind::IGNITION, range::SPELL_TRAP);
        assert!(!hand_condition_allows(LAUNCHER, &revive, 1));
        assert!(hand_condition_allows(LAUNCHER, &revive, 0));
        let send = effect(LAUNCHER, 0, 0, kind::IGNITION, range::SPELL_TRAP);
        assert!(hand_condition_allows(LAUNCHER, &send, 5));
        let install = EffectDescriptor { effect_type: kind::ACTIVATE, ..EffectDescriptor::default() };
        assert!(hand_condition_allows(LAUNCHER, &install, 5));
        let position = effect(NECROMANCER, 0, event::SUMMON_SUCCESS, kind::TRIGGER, 0);
        assert!(hand_condition_allows(NECROMANCER, &position, 5));
        let draw = effect(ARCHFIEND, 0, 1110, kind::TRIGGER, range::HAND);
        assert!(hand_condition_allows(ARCHFIEND, &draw, 1));
        let search = effect(ARCHFIEND, 1, event::SPECIAL_SUMMON_SUCCESS, kind::TRIGGER, 0);
        assert!(!hand_condition_allows(ARCHFIEND, &search, 1));
        let ragin = effect(RAGIN, 0, event::SPECIAL_SUMMON_SUCCESS, kind::TRIGGER, 0);
        assert!(hand_condition_allows(RAGIN, &ragin, 1));
        assert!(!hand_condition_allows(RAGIN, &ragin, 2));
        assert!(hand_condition_allows(900008000, &revive, 1));
        let wrong_slot = effect(MIRAGE, 0, event::SUMMON_SUCCESS, kind::TRIGGER, 0);
        assert_eq!(hand_requirement(MIRAGE, &wrong_slot), None);
    }

    fn observation() -> Observation {
        Observation { me: 0, turn: 2, turn_player: Some(0), phase: Some(Phase::Main1),
            life_points: [8000, 8000], cards: vec![], pile_sizes: vec![], chain: vec![],
            battle_attacker: None, battle_target: None, event_cards: vec![], summon_used: true,
            chain_known: true, can_attack_known: true, coin_toss: None }
    }

    fn view(code: u32, location: Location, sequence: u32) -> CardView {
        CardView { at: CardRef { controller: 0, location, sequence }, code: Some(code),
            position: Position::FACE_UP_ATTACK, attack: 1200, defense: 1000, level: 4,
            can_attack: true, battles: 0, counters: 0, coin_effect: None }
    }

    fn member(card: &CardView) -> Member {
        Member { at: card.at, code: card.code, value: 0, required: false }
    }

    fn choice(kind: ChoiceKind, card: &CardView) -> Choice {
        Choice { kind, card: Some(member(card)), members: vec![], description: 0, place: None }
    }

    fn decision(hint: Hint) -> Decision {
        Decision { kind: DecisionKind::Idle, hint, minimum: 0, maximum: 0,
            selected: vec![], subject: None, choices: vec![] }
    }

    #[test]
    fn native_ragin_draw_count_requires_the_right_summon_and_available_cards() {
        let cards = MemoryCards::default();
        let mut memory = Memory::default();
        let prompt = decision(Hint::None);
        let mut summon = choice(ChoiceKind::SpecialSummon, &view(RAGIN, Location::Extra, 0));
        for (hand, deck, drained, expected) in [
            (0, 2, false, 2), (1, 1, false, 1), (2, 10, false, 0),
            (3, 10, false, 0), (0, 1, false, 0), (1, 0, false, 0),
            (0, 10, true, 0),
        ] {
            let mut obs = observation();
            obs.pile_sizes = vec![(0, Location::Hand, hand), (0, Location::Deck, deck)];
            if drained { obs.cards.push(view(crate::knowledge::SKILL_DRAIN, Location::SpellTrapZone, 0)); }
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
            assert_eq!(extra_summon_draws(&turn, &summon), Some(expected),
                "hand={hand}, deck={deck}, drained={drained}");
        }
        for (hand, opponent_hand, location, position, expected) in [
            (0, 0, Location::MonsterZone, Position::FACE_UP_ATTACK, 0),
            (1, 1, Location::MonsterZone, Position::FACE_UP_ATTACK, 0),
            (0, 1, Location::MonsterZone, Position::FACE_UP_ATTACK, 2),
            (1, 0, Location::MonsterZone, Position::FACE_UP_ATTACK, 1),
            (0, 0, Location::MonsterZone, Position::FACE_DOWN_DEFENSE, 2),
            (0, 0, Location::Graveyard, Position::FACE_UP_ATTACK, 2),
        ] {
            let mut obs = observation();
            obs.pile_sizes = vec![(0, Location::Hand, hand), (1, Location::Hand, opponent_hand),
                (0, Location::Deck, 10)];
            let mut lock = view(crate::knowledge::FABLED_UNICORE, location, 0);
            lock.at.controller = 1;
            lock.position = position;
            obs.cards.push(lock);
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
            assert_eq!(extra_summon_draws(&turn, &summon), Some(expected),
                "hand={hand}/{opponent_hand}, Unicore={location:?}/{position:?}");
        }
        let mut obs = observation();
        obs.pile_sizes.push((0, Location::Deck, 10));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        summon.card.as_mut().unwrap().at.location = Location::Graveyard;
        assert_eq!(extra_summon_draws(&turn, &summon), None);
        summon.card.as_mut().unwrap().at.location = Location::Extra;
        summon.kind = ChoiceKind::Activate;
        assert_eq!(extra_summon_draws(&turn, &summon), None);
        summon.kind = ChoiceKind::SpecialSummon;
        summon.description = description(RAGIN, 0);
        assert_eq!(extra_summon_draws(&turn, &summon), None);
        summon.description = 0;
        summon.card.as_mut().unwrap().code = Some(900008050);
        assert_eq!(extra_summon_draws(&turn, &summon), None);
        assert_eq!(extra_summon_payoff(900008050), None);
    }

    #[test]
    fn discard_payoffs_do_not_apply_to_mills_materials_tributes_or_ordinary_sends() {
        let obs = observation();
        let cards = MemoryCards::default();
        let mut memory = Memory::default();
        for code in [CERBURREL, GANASHIA, LURRIE, KRUS, CATSITH] {
            let descriptor = effect(code, 0, event::TO_GRAVE, kind::TRIGGER, 0);
            assert_eq!(arrival_requirement(code, &descriptor), Some(ArrivalRequirement::DiscardedFromHand));
            for (hint, location) in [(Hint::ToGraveyard, Location::Deck),
                (Hint::ToGraveyard, Location::Hand), (Hint::FusionMaterial, Location::MonsterZone),
                (Hint::SynchroMaterial, Location::MonsterZone), (Hint::Tribute, Location::MonsterZone),
                (Hint::Discard, Location::Deck)]
            {
                let prompt = decision(hint);
                let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
                assert_eq!(arrival_value(&turn, &member(&view(code, location, 0))), Some(0.0));
            }
        }
        let prompt = decision(Hint::Discard);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(arrival_value(&turn, &member(&view(CERBURREL, Location::Hand, 0))).unwrap() > 0.0);
        assert_eq!(arrival_value(&turn, &member(&view(900008001, Location::Hand, 0))), None);
    }

    #[test]
    fn summon_payoffs_require_room_after_the_current_discard_outlet_resolves() {
        let mut obs = observation();
        let cards = MemoryCards::default();
        let prompt = decision(Hint::Discard);
        let mut memory = Memory::default();
        for sequence in 0..4 { obs.cards.push(view(900008010, Location::MonsterZone, sequence)); }
        obs.cards.push(view(CHAWA, Location::Hand, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        let fuel = member(&view(CERBURREL, Location::Hand, 1));
        assert!(arrival_value(&turn, &fuel).unwrap() > 0.0);
        memory.last_activated = Some(CHAWA);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(arrival_value(&turn, &fuel), Some(0.0));
        obs.cards.push(view(900008010, Location::MonsterZone, 4));
        memory.last_activated = None;
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(arrival_value(&turn, &fuel), Some(0.0));
    }

    #[test]
    fn a_discard_redirected_to_banishment_has_no_graveyard_trigger_value() {
        let mut obs = observation();
        obs.cards.push(view(crate::knowledge::MACRO_COSMOS, Location::SpellTrapZone, 0));
        let cards = MemoryCards::default();
        let prompt = decision(Hint::Discard);
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(arrival_value(&turn, &member(&view(CERBURREL, Location::Hand, 0))), Some(0.0));
        obs.cards[0].position = Position::FACE_DOWN_DEFENSE;
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(arrival_value(&turn, &member(&view(CERBURREL, Location::Hand, 0))).unwrap() > 0.0);
    }

    #[test]
    fn krus_can_revive_another_krus_and_catsith_does_not_count_pending_removal() {
        let mut obs = observation();
        let cards = MemoryCards([(KRUS, CardData { code: KRUS, kind: types::MONSTER,
            level: 2, setcodes: vec![FABLED_SET], ..CardData::default() })].into());
        let prompt = decision(Hint::Discard);
        let mut memory = Memory::default();
        let krus = member(&view(KRUS, Location::Hand, 0));
        let catsith = member(&view(CATSITH, Location::Hand, 1));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(arrival_value(&turn, &krus), Some(0.0));
        assert_eq!(arrival_value(&turn, &catsith), Some(0.0));
        obs.cards.push(view(KRUS, Location::Graveyard, 0));
        let mut enemy = view(900008020, Location::MonsterZone, 0);
        enemy.at.controller = 1;
        obs.cards.push(enemy.clone());
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(arrival_value(&turn, &krus).unwrap() > 0.0);
        assert!(arrival_value(&turn, &catsith).unwrap() > 0.0);
        memory.intent.push((enemy.at, enemy.code));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(arrival_value(&turn, &catsith), Some(0.0));
        let mut own_body = view(900008021, Location::MonsterZone, 1);
        own_body.attack = 2400;
        obs.cards.push(own_body);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(arrival_value(&turn, &catsith).unwrap() < 0.0);
    }

    fn fabled_cards() -> MemoryCards {
        MemoryCards([CHAWA, NOZOOCHEE, CERBURREL, KRUS, CATSITH].into_iter()
            .map(|code| (code, CardData { code, kind: types::MONSTER, level: 2,
                setcodes: vec![FABLED_SET], ..CardData::default() })).collect())
    }

    #[test]
    fn hand_outlets_advance_only_the_exact_offered_effect_with_profitable_fuel() {
        let cards = fabled_cards();
        let mut memory = Memory::default();
        for code in [CHAWA, NOZOOCHEE] {
            let mut obs = observation();
            let source = view(code, Location::Hand, 0);
            obs.cards = vec![source.clone(), view(CERBURREL, Location::Hand, 1),
                view(900008040, Location::Hand, 2)];
            let mut activation = choice(ChoiceKind::Activate, &source);
            activation.description = description(code, 0);
            let mut prompt = decision(Hint::None);
            prompt.choices = vec![activation.clone(), choice(ChoiceKind::NormalSummon, &source),
                choice(ChoiceKind::SetMonster, &source)];
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
            assert!(activate_before_normal(&turn, &activation));
            assert!(!activate_before_normal(&turn, &prompt.choices[1]));
            assert!(!activate_before_normal(&turn, &prompt.choices[2]));
            activation.description = description(code, 1);
            assert!(!activate_before_normal(&turn, &activation));
            activation.description = description(code, 0);
            activation.card.as_mut().unwrap().at.location = Location::MonsterZone;
            assert!(!activate_before_normal(&turn, &activation));
            activation.card.as_mut().unwrap().at.location = Location::Hand;
            obs.cards.remove(1);
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
            assert!(!activate_before_normal(&turn, &activation));
        }
    }

    #[test]
    fn hand_outlet_does_not_advance_dead_revival_or_mandatory_self_destruction() {
        let cards = fabled_cards();
        let mut obs = observation();
        let source = view(NOZOOCHEE, Location::Hand, 0);
        obs.cards = vec![source.clone(), view(KRUS, Location::Hand, 1)];
        let mut activation = choice(ChoiceKind::Activate, &source);
        activation.description = description(NOZOOCHEE, 0);
        let mut prompt = decision(Hint::None);
        prompt.choices.push(activation.clone());
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
        obs.cards.push(view(KRUS, Location::Graveyard, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(activate_before_normal(&turn, &activation));
        obs.cards.pop();
        obs.cards[1] = view(CATSITH, Location::Hand, 1);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
        obs.cards.push(view(900008041, Location::MonsterZone, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
        let mut enemy = view(900008042, Location::MonsterZone, 0);
        enemy.at.controller = 1;
        obs.cards.push(enemy.clone());
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(activate_before_normal(&turn, &activation));
        memory.intent.push((enemy.at, enemy.code));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
    }

    #[test]
    fn hand_outlet_reserves_its_zone_and_requires_the_discard_to_reach_graveyard() {
        let cards = fabled_cards();
        let mut obs = observation();
        let source = view(CHAWA, Location::Hand, 0);
        obs.cards = vec![source.clone(), view(CERBURREL, Location::Hand, 1)];
        let mut activation = choice(ChoiceKind::Activate, &source);
        activation.description = description(CHAWA, 0);
        let mut prompt = decision(Hint::None);
        prompt.choices.push(activation.clone());
        let mut memory = Memory::default();
        obs.cards.push(view(crate::knowledge::MACRO_COSMOS, Location::SpellTrapZone, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
        obs.cards.pop();
        for sequence in 0..4 { obs.cards.push(view(900008043, Location::MonsterZone, sequence)); }
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!activate_before_normal(&turn, &activation));
        obs.cards.pop();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(activate_before_normal(&turn, &activation));
    }

    #[test]
    fn set_backrow_before_live_empty_hand_engine_or_offered_ragin_summon() {
        let spell = view(900008030, Location::Hand, 0);
        let setting = choice(ChoiceKind::SetSpellTrap, &spell);
        let mut obs = observation();
        obs.cards.push(spell);
        let cards = MemoryCards::default();
        let mut prompt = decision(Hint::None);
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!set_before_payoff(&turn, &setting));
        obs.cards.push(view(GUARDIAN, Location::MonsterZone, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(set_before_payoff(&turn, &setting));
        obs.cards.pop();
        obs.cards.push(view(900008031, Location::Hand, 1));
        prompt.choices.push(choice(ChoiceKind::SpecialSummon, &view(RAGIN, Location::Extra, 0)));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(set_before_payoff(&turn, &setting));
        prompt.choices[0].card.as_mut().unwrap().at.location = Location::Graveyard;
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert!(!set_before_payoff(&turn, &setting));
    }
}
