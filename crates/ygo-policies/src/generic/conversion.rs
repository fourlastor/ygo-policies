//! Resource accounting for material-conversion effects. These card facts
//! cover sources present in the supplied decks; their rules describe effect
//! transactions, never a deck or a chosen specialist policy.
//!
//! Source: the corresponding `vendor/CardScripts/official/c<code>.lua` and
//! `proc_fusion_spell.lua`. The engine still decides material legality. This
//! module supplies conservative ordering and resource-preservation priors.

use super::Generic;
use crate::agent::Turn;
use crate::cards::types;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

const POLYMERIZATION: u32 = 24094653;
const POLYMERIZATION_ART: u32 = 27847700;
const GEM_KNIGHT_FUSION: u32 = 1264319;
const FUSION_GATE: u32 = 33550694;
const MIRACLE_FUSION: u32 = 45906428;
const MIRACLE_SYNCHRO_FUSION: u32 = 36484016;
const PARALLEL_WORLD_FUSION: u32 = 54283059;
const SUPER_POLYMERIZATION: u32 = 48130397;
const FUTURE_FUSION: u32 = 77565204;
const ABSOLUTE_ZERO: u32 = 40854197;
const AQUAMARINE: u32 = 13108445;

/// Active-pool cards with at least one explicit conversion fact here. This
/// is an auditable coverage list, not a claim to model every effect on them.
pub const SUPPORTED_CODES: &[u32] = &[
    POLYMERIZATION, POLYMERIZATION_ART, GEM_KNIGHT_FUSION, FUSION_GATE,
    MIRACLE_FUSION, MIRACLE_SYNCHRO_FUSION, PARALLEL_WORLD_FUSION,
    SUPER_POLYMERIZATION, FUTURE_FUSION, ABSOLUTE_ZERO, AQUAMARINE,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Materials {
    /// Cards in hand/on the field become graveyard resources.
    HandFieldToGrave,
    /// Fusion Gate consumes live resources permanently.
    HandFieldBanish,
    /// Miracle effects can use already-spent graveyard cards.
    FieldGraveBanish,
    /// Parallel World Fusion shuffles banished cards back.
    BanishedToDeck,
    /// Super Polymerization may remove opposing monsters.
    BothFieldsToGrave,
    /// Future Fusion selects materials from the Deck.
    DeckToGrave,
}

fn materials(code: u32) -> Option<Materials> {
    Some(match code {
        POLYMERIZATION | POLYMERIZATION_ART | GEM_KNIGHT_FUSION => Materials::HandFieldToGrave,
        FUSION_GATE => Materials::HandFieldBanish,
        MIRACLE_FUSION | MIRACLE_SYNCHRO_FUSION => Materials::FieldGraveBanish,
        PARALLEL_WORLD_FUSION => Materials::BanishedToDeck,
        SUPER_POLYMERIZATION => Materials::BothFieldsToGrave,
        FUTURE_FUSION => Materials::DeckToGrave,
        _ => return None,
    })
}

/// These two field-departure effects are valuable precisely when the body
/// is consumed. Their script conditions differ: Aquamarine must reach the
/// graveyard, whereas Absolute Zero also triggers after being banished.
fn departure_value(t: &Turn, card: &CardView, route: Option<Materials>) -> f64 {
    if card.at.location != Location::MonsterZone || !card.position.face_up { return 0.0; }
    let Some(code) = card.code.map(|code| t.ctx.canonical(code)) else { return 0.0; };
    let sends_to_grave = matches!(route,
        Some(Materials::HandFieldToGrave | Materials::BothFieldsToGrave));
    let banishes = matches!(route,
        Some(Materials::HandFieldBanish | Materials::FieldGraveBanish));
    match code {
        ABSOLUTE_ZERO if sends_to_grave || banishes => t.ctx.monsters(t.ctx.opp).into_iter()
            .filter(|enemy| t.ctx.reaches(enemy, code, false, true))
            .map(|enemy| 1000.0 + t.ctx.threat(enemy).max(0) as f64).sum(),
        AQUAMARINE if sends_to_grave => t.ctx.obs.cards.iter()
            .filter(|enemy| enemy.at.controller == t.ctx.opp && enemy.at.location.is_field()
                && t.ctx.reaches(enemy, code, true, false))
            .map(|enemy| 1000.0 + t.ctx.threat(enemy).max(0) as f64)
            .fold(0.0, f64::max),
        _ => 0.0,
    }
}

/// `Some(false)` is a pilot prior, not a legality claim: engine search can
/// still examine the action. Match the particular effect, not merely its
/// card, so installation, recovery and triggered effects remain independent.
pub(super) fn allow_activation(t: &Turn, choice: &Choice) -> Option<bool> {
    if choice.kind != ChoiceKind::Activate { return None; }
    let at = choice.at()?;
    if at.controller != t.ctx.me { return None; }
    let code = t.ctx.canonical(choice.code()?);
    if code == GEM_KNIGHT_FUSION && at.location == Location::Graveyard
        && choice.description == (((code as u64) << 20) | 1)
    {
        // Recovering a second copy costs another Gem-Knight banish without
        // making a fusion available that the copy already in hand could not.
        return Some(!t.ctx.in_hand(GEM_KNIGHT_FUSION));
    }
    let route = materials(code)?;
    let immediate_fusion = match route {
        Materials::HandFieldBanish => at.location == Location::SpellTrapZone
            && choice.description == (code as u64) << 20,
        Materials::DeckToGrave => false,
        _ => matches!(at.location, Location::Hand | Location::SpellTrapZone)
            && choice.description == 1170,
    };
    if !immediate_fusion { return None; }
    if !matches!(route, Materials::HandFieldToGrave | Materials::HandFieldBanish) {
        return None;
    }
    // Re-fusing an established winning Fusion is a recurring resource sink.
    // Grave/banished-material routes and opposing-material removal are not
    // equivalent exchanges and deliberately do not use this conservative
    // prior. No opponent deck or hidden card identity is consulted.
    let obstacle = t.ctx.monsters(t.ctx.opp).into_iter()
        .map(|card| t.ctx.battle_stat(card)).max().unwrap_or(0);
    let established: Vec<_> = t.ctx.monsters(t.ctx.me).into_iter().filter(|card| {
        card.position.face_up && t.ctx.view_data(card).is(types::FUSION)
            && card.attack > obstacle && card.attack >= 2000
            && departure_value(t, card, Some(route)) == 0.0
    }).map(|card| card.at).collect();
    if established.is_empty() { return Some(true); }
    // Before activation the engine has not exposed the chosen Fusion's
    // recipe. Two other visible monsters are only plausible materials, but
    // they are enough to leave an independent second Fusion available to
    // the pilot. Once the exact legal materials are offered, preserve the
    // established body through material_score rather than pretending to
    // know a hidden procedure here.
    let spare = t.ctx.obs.cards.iter().filter(|card| {
        card.at.controller == t.ctx.me
            && matches!(card.at.location, Location::Hand | Location::MonsterZone)
            && t.ctx.view_data(card).is_monster()
            && !established.contains(&card.at)
    }).take(2).count();
    Some(spare >= 2)
}

/// Material costs depend on where a card is now. Banishing a spent graveyard
/// monster is cheaper than losing a live attacker; sending a card to the
/// graveyard may create an action, while banishing it cannot earn that same
/// graveyard-action credit.
pub(super) fn material_score(t: &Turn, member: &Member) -> Option<f64> {
    if t.decision.hint != Hint::FusionMaterial { return None; }
    let code = member.code?;
    let ctx = t.ctx;
    if member.at.controller != ctx.me {
        return Some(5000.0 + ctx.card(member.at).map_or(1500, |card| ctx.threat(card)) as f64);
    }
    let route = t.memory.last_activated.and_then(|source| materials(ctx.canonical(source)));
    let sends_to_grave = matches!(route,
        Some(Materials::HandFieldToGrave | Materials::BothFieldsToGrave | Materials::DeckToGrave));
    let grave = Generic::grave_utility(&ctx, code);
    let worth = Generic::body_value(&ctx, code) as f64;
    let destination_credit = if sends_to_grave { grave } else { 0.0 };
    Some(match member.at.location {
        Location::Graveyard => -100.0 - worth * 0.1 - grave,
        Location::Banished => -100.0 - worth * 0.05,
        Location::Deck => destination_credit - worth * 0.1,
        Location::MonsterZone => {
            let live = ctx.card(member.at).map_or(worth, |card| {
                card.attack.max(card.defense).max(0) as f64
                    + if ctx.data(code).is(types::EFFECT) { 350.0 } else { 0.0 }
            });
            let payoff = ctx.card(member.at).map_or(0.0, |card| departure_value(t, card, route));
            destination_credit.max(payoff) - live - if ctx.data(code).is_extra() { 2200.0 } else { 400.0 }
        }
        _ => destination_credit - worth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{effect_categories as cat, effect_locations as range, effect_types,
        CardData, EffectDescriptor, MemoryCards, ProfiledCards};
    use crate::ctx::Ctx;
    use crate::model::{CardRef, CardView, Decision, DecisionKind, Observation, Phase, Position};
    use std::sync::Arc;

    fn observation() -> Observation {
        Observation {
            me: 0, turn: 2, turn_player: Some(0), phase: Some(Phase::Main1),
            life_points: [8000, 8000], cards: vec![], pile_sizes: vec![], chain: vec![],
            battle_attacker: None, battle_target: None, event_cards: vec![], summon_used: true,
            chain_known: true, can_attack_known: true, coin_toss: None,
        }
    }

    fn view(code: u32, controller: u8, location: Location, attack: i32) -> CardView {
        CardView {
            at: CardRef { controller, location, sequence: 0 }, code: Some(code),
            position: Position::FACE_UP_ATTACK, attack, defense: 1000, level: 4,
            can_attack: true, battles: 0, counters: 0, coin_effect: None, negated: false,
        }
    }

    fn member(card: &CardView) -> Member {
        Member { at: card.at, code: card.code, value: 0, required: false }
    }

    fn decision() -> Decision {
        Decision { kind: DecisionKind::Idle, hint: Hint::None, minimum: 0, maximum: 0,
            selected: vec![], subject: None, choices: vec![] }
    }

    fn choice(code: u32, location: Location, description: u64) -> Choice {
        Choice { kind: ChoiceKind::Activate, card: Some(member(&view(code, 0, location, 0))),
            members: vec![], description, place: None }
    }

    #[test]
    fn existing_fusion_stops_live_resource_cycles_but_not_recycled_materials() {
        let boss = 900009001;
        let cards = MemoryCards([(boss, CardData { code: boss,
            kind: types::MONSTER | types::FUSION, attack: 2800, ..CardData::default() })].into());
        let mut obs = observation();
        obs.cards.push(view(boss, 0, Location::MonsterZone, 2800));
        obs.cards.push(view(900009002, 1, Location::MonsterZone, 1800));
        let prompt = decision();
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        for code in [POLYMERIZATION, POLYMERIZATION_ART, GEM_KNIGHT_FUSION] {
            assert_eq!(allow_activation(&turn, &choice(code, Location::Hand, 1170)), Some(false));
        }
        assert_eq!(allow_activation(&turn, &choice(FUSION_GATE, Location::SpellTrapZone,
            (FUSION_GATE as u64) << 20)), Some(false));
        for code in [MIRACLE_FUSION, MIRACLE_SYNCHRO_FUSION, PARALLEL_WORLD_FUSION, SUPER_POLYMERIZATION] {
            assert_eq!(allow_activation(&turn, &choice(code, Location::Hand, 1170)), None);
        }
        assert_eq!(allow_activation(&turn, &choice(FUSION_GATE, Location::Hand, 0)), None);
        assert_eq!(allow_activation(&turn, &choice(FUTURE_FUSION, Location::Hand, 0)), None);
        assert_eq!(allow_activation(&turn, &choice(900009003, Location::Hand, 1170)), None);
    }

    #[test]
    fn fusion_is_available_to_establish_or_replace_an_outmatched_board() {
        let boss = 900009011;
        let cards = MemoryCards([(boss, CardData { code: boss,
            kind: types::MONSTER | types::FUSION, attack: 2200, ..CardData::default() })].into());
        for with_boss in [false, true] {
            let mut obs = observation();
            if with_boss { obs.cards.push(view(boss, 0, Location::MonsterZone, 2200)); }
            obs.cards.push(view(900009012, 1, Location::MonsterZone, 2600));
            let prompt = decision();
            let mut memory = Memory::default();
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
            assert_eq!(allow_activation(&turn, &choice(GEM_KNIGHT_FUSION, Location::Hand, 1170)), Some(true));
        }
    }

    #[test]
    fn an_established_fusion_does_not_prevent_a_second_fusion_from_spare_materials() {
        let boss = 900009015;
        let small = 900009016;
        let cards = MemoryCards([
            (boss, CardData { code: boss, kind: types::MONSTER | types::FUSION,
                attack: 2800, ..CardData::default() }),
            (small, CardData { code: small, kind: types::MONSTER,
                attack: 1200, ..CardData::default() }),
        ].into());
        let mut obs = observation();
        obs.cards = vec![view(boss, 0, Location::MonsterZone, 2800),
            view(small, 0, Location::Hand, 1200), view(small, 0, Location::Hand, 1200)];
        obs.cards[2].at.sequence = 1;
        let prompt = decision();
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(allow_activation(&turn, &choice(POLYMERIZATION, Location::Hand, 1170)), Some(true));
    }

    #[test]
    fn gem_fusion_does_not_banish_another_card_for_a_duplicate_hand_copy() {
        let cards = MemoryCards::default();
        let mut obs = observation();
        let prompt = decision();
        let mut memory = Memory::default();
        let recovery = choice(GEM_KNIGHT_FUSION, Location::Graveyard,
            ((GEM_KNIGHT_FUSION as u64) << 20) | 1);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(allow_activation(&turn, &recovery), Some(true));
        obs.cards.push(view(GEM_KNIGHT_FUSION, 0, Location::Hand, 0));
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(allow_activation(&turn, &recovery), Some(false));
    }

    #[test]
    fn material_selection_spends_opponent_and_grave_cards_before_live_bosses() {
        let boss = 900009021;
        let small = 900009022;
        let cards = MemoryCards([
            (boss, CardData { code: boss, kind: types::MONSTER | types::FUSION,
                attack: 2800, ..CardData::default() }),
            (small, CardData { code: small, kind: types::MONSTER,
                attack: 1200, ..CardData::default() }),
        ].into());
        let mut obs = observation();
        obs.cards = vec![view(boss, 0, Location::MonsterZone, 2800),
            view(boss, 0, Location::Graveyard, 2800), view(small, 0, Location::Hand, 1200),
            view(small, 1, Location::MonsterZone, 1200)];
        let mut prompt = decision();
        prompt.hint = Hint::FusionMaterial;
        let mut memory = Memory::default();
        memory.last_activated = Some(SUPER_POLYMERIZATION);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        let scores: Vec<f64> = obs.cards.iter().map(|card| material_score(&turn, &member(card)).unwrap()).collect();
        assert!(scores[3] > scores[1] && scores[1] > scores[2] && scores[2] > scores[0], "{scores:?}");
    }

    #[test]
    fn banishing_material_never_earns_its_graveyard_action_credit() {
        let code = 900009031;
        let cards = ProfiledCards::new(Arc::new(MemoryCards([(code, CardData {
            code, kind: types::MONSTER, attack: 500, ..CardData::default()
        })].into())), [(code, vec![EffectDescriptor {
            category: cat::SPECIAL_SUMMON, effect_type: effect_types::IGNITION,
            range: range::GRAVEYARD, ..EffectDescriptor::default()
        }])].into());
        let mut obs = observation();
        obs.cards.push(view(code, 0, Location::Hand, 500));
        let mut prompt = decision();
        prompt.hint = Hint::FusionMaterial;
        let mut memory = Memory::default();
        memory.last_activated = Some(POLYMERIZATION);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        let sent = material_score(&turn, &member(&obs.cards[0])).unwrap();
        memory.last_activated = Some(FUSION_GATE);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        let banished = material_score(&turn, &member(&obs.cards[0])).unwrap();
        assert!(sent > banished + 2000.0);
    }

    #[test]
    fn field_departure_removal_is_credited_only_for_its_actual_destination() {
        let enemy = 900009041;
        let cards = MemoryCards([
            (ABSOLUTE_ZERO, CardData { code: ABSOLUTE_ZERO,
                kind: types::MONSTER | types::FUSION | types::EFFECT,
                attack: 2500, ..CardData::default() }),
            (AQUAMARINE, CardData { code: AQUAMARINE,
                kind: types::MONSTER | types::FUSION | types::EFFECT,
                attack: 1400, ..CardData::default() }),
            (enemy, CardData { code: enemy, kind: types::MONSTER,
                attack: 1800, ..CardData::default() }),
        ].into());
        let mut obs = observation();
        obs.cards = vec![view(ABSOLUTE_ZERO, 0, Location::MonsterZone, 2500),
            view(enemy, 1, Location::MonsterZone, 1800)];
        let prompt = decision();
        let mut memory = Memory::default();
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &prompt, memory: &mut memory };
        assert_eq!(allow_activation(&turn, &choice(POLYMERIZATION, Location::Hand, 1170)), Some(true));
        assert!(departure_value(&turn, &obs.cards[0], Some(Materials::HandFieldBanish)) > 0.0);
        let aqua = view(AQUAMARINE, 0, Location::MonsterZone, 1400);
        assert!(departure_value(&turn, &aqua, Some(Materials::HandFieldToGrave)) > 0.0);
        assert_eq!(departure_value(&turn, &aqua, Some(Materials::HandFieldBanish)), 0.0);
        let grave_aqua = view(AQUAMARINE, 0, Location::Graveyard, 1400);
        assert_eq!(departure_value(&turn, &grave_aqua, Some(Materials::HandFieldToGrave)), 0.0);
    }
}
