//! Position changes which can open a source-backed effect in the main phase.
//!
//! These are public-state priors for individual cards in the supplied pool.
//! They do not inspect a deck list or promise that a matching hidden Deck
//! target exists. The engine must offer the position change, and subsequently
//! decides whether the resulting trigger or ignition effect is legal.

use crate::agent::Turn;
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, DecisionKind, Location, Phase};

const TANNGNJOSTR: u32 = 14677495;
const REMOTEN: u32 = 57108202;
const KRISTYA: u32 = 59509952;
const NECROVALLEY: u32 = 47355498;
const KYCOO: u32 = 88240808;
const RAI_OH: u32 = 71564252;
const NEEDLE_WORM: u32 = 81843628;
const CARTAROS: u32 = 51043243;
const SCAPEGOAT: u32 = 73915051;
const FIRES_OF_DOOMSDAY: u32 = 46173679;

/// Each entry has at least one timing or public-lock fact here; this does not
/// claim complete knowledge of its effects. All are in the supplied pool.
pub const SUPPORTED_CODES: &[u32] = &[
    TANNGNJOSTR, REMOTEN, KRISTYA, NECROVALLEY, KYCOO, RAI_OH,
    NEEDLE_WORM, CARTAROS, SCAPEGOAT, FIRES_OF_DOOMSDAY,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttackAccess {
    /// Optional trigger when this card changes from Defense to face-up Attack.
    RecruitOtherFromDeck,
    /// Attack-position ignition: banish a matching monster, then search a
    /// monster of the same set and level. Only GY fuel is valued here.
    BanishGraveToSearchSameLevel { setcode: u16 },
    FlipSearch,
    FlipMillOpponent { count: u32 },
}

fn attack_access(code: u32) -> Option<AttackAccess> {
    match code {
        // c14677495.deckspcon/decksptg: previous POS_DEFENSE includes facedown
        // Defense. OCGCore operations.cpp also raises EVENT_CHANGE_POS on a
        // Flip Summon, so that legal transition can access this trigger.
        TANNGNJOSTR => Some(AttackAccess::RecruitOtherFromDeck),
        // c57108202.cona/filtera/tga: own GY Morphtronic, positive level,
        // removable, and a same-level Morphtronic available in the Deck.
        REMOTEN => Some(AttackAccess::BanishGraveToSearchSameLevel { setcode: 0x26 }),
        // c51043243.tg/op: optional matching target is unknown until resolution.
        CARTAROS => Some(AttackAccess::FlipSearch),
        // c81843628.operation: send the opponent's top five cards to the GY.
        // Emptying that Deck is not itself a win; a later failed draw is needed.
        NEEDLE_WORM => Some(AttackAccess::FlipMillOpponent { count: 5 }),
        _ => None,
    }
}

fn face_up_monster(ctx: &Ctx, player: u8, code: u32) -> bool {
    ctx.monsters(player).into_iter().any(|card| card.position.face_up && ctx.is(card, code))
}

fn grave_search_locked(ctx: &Ctx) -> bool {
    // c47355498 e2a/e2b: either active Field Spell prevents GY banishment.
    // Conservatively omit rare immunity exceptions, rather than promise fuel.
    let valley = [ctx.me, ctx.opp].into_iter().any(|player| {
        ctx.spell_traps(player).into_iter().any(|card| card.at.sequence == 5
            && card.position.face_up && ctx.is(card, NECROVALLEY))
    });
    // c88240808 e2/rmlimit: only the opponent of Kycoo's controller is locked.
    // c71564252 e1: either player's face-up Rai-Oh prevents Deck-to-hand moves.
    valley || face_up_monster(ctx, ctx.opp, KYCOO) || deck_search_locked(ctx)
}

fn deck_search_locked(ctx: &Ctx) -> bool {
    [ctx.me, ctx.opp].into_iter().any(|player| face_up_monster(ctx, player, RAI_OH))
}

/// An early FLIP effect should not expose a weak body to a known larger
/// attacker just for a possible search or mill. Other own attackers do not
/// shield this card in Yu-Gi-Oh. Unknown opposing monsters remain a risk.
fn flip_exposure_is_small(ctx: &Ctx, source: &CardView) -> bool {
    ctx.monsters(ctx.opp).into_iter().all(|enemy| enemy.known() && enemy.attack <= source.attack)
}

/// Do not trade a Defense Position for a plainly lethal attack target just
/// for potential effect access. A new friendly body does not shield this one.
/// This conservative prior does not assume future removal or hidden protection.
fn fatal_attack_exposure(ctx: &Ctx, source: &CardView) -> bool {
    ctx.monsters(ctx.opp).into_iter().any(|enemy| enemy.position.face_up && enemy.known()
        && enemy.attack.saturating_sub(source.attack.max(0)) >= ctx.my_lp())
}

/// Whether an actually offered Defense-to-Attack change has a plausible
/// immediate effect payoff. Callers still apply their ordinary freshness
/// guard before choosing it. No reverse change to Defense receives priority.
pub(super) fn change_position_for_access(t: &Turn, choice: &Choice) -> bool {
    let ctx = t.ctx;
    if t.decision.kind != DecisionKind::Idle || choice.kind != ChoiceKind::ChangePosition
        || !t.decision.choices.contains(choice) || !ctx.my_turn()
        || !matches!(ctx.phase(), Some(Phase::Main1 | Phase::Main2))
        || choice.at().is_none_or(|at| at.controller != ctx.me || at.location != Location::MonsterZone)
    {
        return false;
    }
    let Some(code) = choice.code().map(|code| ctx.canonical(code)) else { return false };
    let Some(access) = attack_access(code) else { return false };
    let Some(source) = choice.at().and_then(|at| ctx.card(at)) else { return false };
    if !ctx.is(source, code) || source.position.attack || ctx.effects_drained() || ctx.wasted(code)
        || fatal_attack_exposure(&ctx, source)
    {
        return false;
    }
    match access {
        AttackAccess::RecruitOtherFromDeck => {
            // c14677495.decksptg needs a free main monster zone. c59509952 e2
            // stops both players Special Summoning while Kristya is face-up.
            ctx.deck_size(ctx.me) > 0 && ctx.free_monster_zones(ctx.me) > 0
                && ![ctx.me, ctx.opp].into_iter().any(|player| face_up_monster(&ctx, player, KRISTYA))
        }
        AttackAccess::BanishGraveToSearchSameLevel { setcode } => {
            ctx.deck_size(ctx.me) > 0 && !grave_search_locked(&ctx) && ctx.graveyard(ctx.me).into_iter().any(|card| {
                let data = ctx.view_data(card);
                card.level > 0 && data.is(types::MONSTER) && data.in_set(setcode)
            })
        }
        AttackAccess::FlipSearch => !source.position.face_up && ctx.deck_size(ctx.me) > 0
            && !deck_search_locked(&ctx) && flip_exposure_is_small(&ctx, source),
        AttackAccess::FlipMillOpponent { count } => !source.position.face_up
            && ctx.deck_size(ctx.opp).min(count) > 0 && flip_exposure_is_small(&ctx, source),
    }
}

/// Avoid installing our own turn-long summon restriction before a visible
/// development opportunity. This is only a timing veto, never a claim that
/// either token spell must be activated at another time.
pub(super) fn allow_activation(t: &Turn, choice: &Choice) -> Option<bool> {
    let ctx = t.ctx;
    let code = ctx.canonical(choice.code()?);
    let description = match code {
        // c73915051.initial_effect has no description; c46173679 uses slot0.
        SCAPEGOAT => 0,
        FIRES_OF_DOOMSDAY => (FIRES_OF_DOOMSDAY as u64) << 20,
        _ => return None,
    };
    if choice.kind != ChoiceKind::Activate || choice.description != description
        || choice.at().is_none_or(|at| at.controller != ctx.me
            || !matches!(at.location, Location::Hand | Location::SpellTrapZone))
        || !t.decision.choices.contains(choice) || !ctx.my_turn()
        || choice.at().and_then(|at| ctx.card(at)).is_none_or(|source| !ctx.is(source, code))
        || ctx.incoming_attack().is_some() || ctx.obs.chain.iter().any(|link| link.controller == ctx.opp)
        || !matches!(ctx.phase(), Some(Phase::Draw | Phase::Standby | Phase::Main1 | Phase::Main2))
    {
        return None;
    }
    let offered_development = t.choices().any(|(index, candidate)| {
        t.fresh(index) && candidate.at().is_some_and(|at| at.controller == ctx.me)
            && (matches!(candidate.kind, ChoiceKind::NormalSummon | ChoiceKind::SpecialSummon)
                || candidate.kind == ChoiceKind::ChangePosition
                    && candidate.at().and_then(|at| ctx.card(at)).is_some_and(|card| !card.position.face_up))
    });
    // Draw/Standby chain windows do not yet offer normal summons. Only a
    // visible, low-level, ordinary main-deck monster is an affordable prior.
    // TYPE_SPSUMMON (0x2000000) marks monsters which cannot be Normal Summoned.
    let normal_in_hand = !ctx.obs.summon_used && ctx.free_monster_zones(ctx.me) > 0
        && ctx.hand().into_iter().any(|card| {
            let data = ctx.view_data(card);
            data.is(types::MONSTER) && !data.is(types::EXTRA | types::RITUAL | 0x2000000)
                && (1..=4).contains(&data.level)
        });
    (offered_development || normal_in_hand).then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{CardData, MemoryCards};
    use crate::model::{CardRef, ChainLink, Decision, Hint, Member, Observation, Position};

    const UNSEEN_FUEL: u32 = 900008101;
    const UNSEEN_OTHER: u32 = 900008102;

    fn cards() -> MemoryCards {
        MemoryCards([(UNSEEN_FUEL, CardData { code: UNSEEN_FUEL,
            kind: types::MONSTER | types::EFFECT, level: 3, setcodes: vec![0x26],
            ..CardData::default() })].into())
    }

    fn view(code: u32, location: Location, sequence: u32) -> CardView {
        CardView { at: CardRef { controller: 0, location, sequence }, code: Some(code),
            position: Position::FACE_UP_ATTACK, attack: 300, defense: 1200, level: 3,
            can_attack: false, battles: 0, counters: 0, coin_effect: None, negated: false }
    }

    fn observation(code: u32) -> Observation {
        let mut source = view(code, Location::MonsterZone, 0);
        source.position = Position::FACE_DOWN_DEFENSE;
        Observation { me: 0, turn: 7, turn_player: Some(0), phase: Some(Phase::Main2),
            life_points: [8000, 8000], cards: vec![source, view(UNSEEN_FUEL, Location::Graveyard, 0)],
            pile_sizes: vec![(0, Location::Deck, 26)], chain: vec![], battle_attacker: None,
            battle_target: None, event_cards: vec![], summon_used: true,
            chain_known: true, can_attack_known: true, coin_toss: None }
    }

    fn choice(card: &CardView) -> Choice {
        Choice { kind: ChoiceKind::ChangePosition,
            card: Some(Member { at: card.at, code: card.code, value: 0, required: false }),
            members: vec![], description: 0, place: None }
    }

    fn prompt(choices: Vec<Choice>) -> Decision {
        Decision { kind: DecisionKind::Idle, hint: Hint::None, minimum: 0, maximum: 0,
            selected: vec![], subject: None, choices }
    }

    fn access(obs: &Observation) -> bool {
        let cards = cards();
        let mut memory = Memory::default();
        let change = choice(&obs.cards[0]);
        let decision = prompt(vec![change.clone()]);
        let turn = Turn { ctx: Ctx::new(obs, &cards), decision: &decision, memory: &mut memory };
        change_position_for_access(&turn, &change)
    }

    #[test]
    fn known_position_sources_work_with_unseen_surrounding_cards() {
        for code in [TANNGNJOSTR, REMOTEN] {
            for position in [Position::FACE_DOWN_DEFENSE, Position::FACE_UP_DEFENSE] {
                let mut obs = observation(code);
                obs.cards[0].position = position;
                obs.cards.push(view(UNSEEN_OTHER, Location::Hand, 0));
                assert!(access(&obs), "{code}, {position:?}");
                obs.phase = Some(Phase::Main1);
                assert!(access(&obs));
            }
        }
        assert!(!access(&observation(UNSEEN_OTHER)));
        assert_eq!(attack_access(UNSEEN_OTHER), None);
    }

    #[test]
    fn only_an_offered_own_monster_defense_to_attack_change_qualifies() {
        let cards = cards();
        let mut memory = Memory::default();
        let obs = observation(TANNGNJOSTR);
        let change = choice(&obs.cards[0]);
        let absent = prompt(vec![]);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &absent, memory: &mut memory };
        assert!(!change_position_for_access(&turn, &change));
        for kind in [ChoiceKind::Activate, ChoiceKind::NormalSummon, ChoiceKind::SetMonster] {
            let mut wrong = change.clone();
            wrong.kind = kind;
            let decision = prompt(vec![wrong.clone()]);
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &decision, memory: &mut memory };
            assert!(!change_position_for_access(&turn, &wrong));
        }
        for position in [Position::FACE_UP_ATTACK, Position::FACE_DOWN_ATTACK] {
            let mut obs = obs.clone();
            obs.cards[0].position = position;
            assert!(!access(&obs));
        }
        for location in [Location::Graveyard, Location::Hand, Location::SpellTrapZone] {
            let mut obs = obs.clone();
            obs.cards[0].at.location = location;
            assert!(!access(&obs));
        }
        let mut wrong_owner = obs.clone();
        wrong_owner.cards[0].at.controller = 1;
        assert!(!access(&wrong_owner));
        let mut stale = change.clone();
        stale.card.as_mut().unwrap().code = Some(REMOTEN);
        let decision = prompt(vec![stale.clone()]);
        let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &decision, memory: &mut memory };
        assert!(!change_position_for_access(&turn, &stale));
    }

    #[test]
    fn main_phase_own_turn_and_nonempty_deck_are_necessary() {
        for code in [TANNGNJOSTR, REMOTEN] {
            let mut obs = observation(code);
            obs.pile_sizes = vec![(0, Location::Deck, 0)];
            assert!(!access(&obs));
            obs.pile_sizes = vec![(0, Location::Deck, 26)];
            obs.turn_player = Some(1);
            assert!(!access(&obs));
            obs.turn_player = Some(0);
            obs.phase = Some(Phase::End);
            assert!(!access(&obs));
        }
    }

    #[test]
    fn source_effect_negation_does_not_justify_exposing_the_monster() {
        for code in [TANNGNJOSTR, REMOTEN] {
            let mut obs = observation(code);
            obs.cards.push(view(crate::knowledge::SKILL_DRAIN, Location::SpellTrapZone, 0));
            assert!(!access(&obs));
            obs.cards.last_mut().unwrap().position = Position::FACE_DOWN_DEFENSE;
            assert!(access(&obs));
            let mut unicore = view(crate::knowledge::FABLED_UNICORE, Location::MonsterZone, 0);
            unicore.at.controller = 1;
            obs.cards.push(unicore);
            assert!(!access(&obs), "equal public hand sizes");
            obs.pile_sizes.push((1, Location::Hand, 1));
            assert!(access(&obs), "unequal hands do not activate Unicore's lock");
        }
    }

    #[test]
    fn recruitment_requires_space_and_no_visible_kristya_lock() {
        let mut obs = observation(TANNGNJOSTR);
        for sequence in 1..5 { obs.cards.push(view(UNSEEN_OTHER, Location::MonsterZone, sequence)); }
        assert!(!access(&obs));
        obs.cards.pop();
        assert!(access(&obs));
        for player in [0, 1] {
            let mut lock = view(KRISTYA, Location::MonsterZone, 4);
            lock.at.controller = player;
            obs.cards.push(lock);
            assert!(!access(&obs));
            obs.cards.last_mut().unwrap().at.location = Location::Graveyard;
            assert!(access(&obs));
            obs.cards.pop();
        }
    }

    #[test]
    fn remoten_needs_own_positive_level_matching_grave_fuel() {
        let mut obs = observation(REMOTEN);
        assert!(access(&obs));
        // Full monster zones do not stop the search: it summons no monster.
        for sequence in 1..5 { obs.cards.push(view(UNSEEN_OTHER, Location::MonsterZone, sequence)); }
        assert!(access(&obs));
        obs.cards[1].level = 0;
        assert!(!access(&obs));
        obs.cards[1].level = 3;
        obs.cards[1].code = Some(UNSEEN_OTHER);
        assert!(!access(&obs));
        obs.cards[1].code = Some(UNSEEN_FUEL);
        obs.cards[1].at.controller = 1;
        assert!(!access(&obs));
        obs.cards[1].at.controller = 0;
        for location in [Location::Hand, Location::Banished, Location::MonsterZone] {
            obs.cards[1].at.location = location;
            assert!(!access(&obs), "do not spend a live body or infer unseen GY fuel");
        }
    }

    #[test]
    fn remoten_respects_banish_and_search_locks_with_exact_ownership_and_location() {
        for (code, player, location, sequence, blocked) in [
            (NECROVALLEY, 0, Location::SpellTrapZone, 5, true),
            (NECROVALLEY, 1, Location::SpellTrapZone, 5, true),
            (NECROVALLEY, 1, Location::SpellTrapZone, 0, false),
            (NECROVALLEY, 1, Location::Graveyard, 5, false),
            (KYCOO, 1, Location::MonsterZone, 1, true),
            (KYCOO, 0, Location::MonsterZone, 1, false),
            (KYCOO, 1, Location::SpellTrapZone, 1, false),
            (RAI_OH, 0, Location::MonsterZone, 1, true),
            (RAI_OH, 1, Location::MonsterZone, 1, true),
            (RAI_OH, 1, Location::Graveyard, 1, false),
        ] {
            let mut obs = observation(REMOTEN);
            let mut lock = view(code, location, sequence);
            lock.at.controller = player;
            obs.cards.push(lock);
            assert_eq!(access(&obs), !blocked, "{code}/{player}/{location:?}");
            obs.cards.last_mut().unwrap().position = Position::FACE_DOWN_DEFENSE;
            assert!(access(&obs), "a facedown card has no continuous lock");
        }
    }

    #[test]
    fn productive_flips_require_a_live_payoff_without_unsafe_exposure() {
        for code in [CARTAROS, NEEDLE_WORM] {
            let mut obs = observation(code);
            obs.pile_sizes.push((1, Location::Deck, 5));
            assert!(access(&obs));
            obs.cards[0].position = Position::FACE_UP_DEFENSE;
            assert!(!access(&obs), "changing a face-up card does not trigger FLIP");
            obs.cards[0].position = Position::FACE_DOWN_DEFENSE;
            let mut enemy = view(UNSEEN_OTHER, Location::MonsterZone, 0);
            enemy.at.controller = 1;
            enemy.attack = 1800;
            obs.cards.push(enemy);
            assert!(!access(&obs), "do not expose this 300 ATK body to 1800 ATK");
            // A second strong friendly body cannot prevent attacks on this one.
            let mut defender = view(UNSEEN_OTHER, Location::MonsterZone, 1);
            defender.attack = 3000;
            obs.cards.push(defender);
            assert!(!access(&obs));
            obs.cards[2].attack = 200;
            assert!(access(&obs));
            obs.cards[2].code = None;
            obs.cards[2].position = Position::FACE_DOWN_DEFENSE;
            assert!(!access(&obs), "unknown opposing stats cannot establish safety");
        }
        // Morphing Jar has disruptive hand replacement, not one of these facts.
        assert!(!access(&observation(33508719)));
    }

    #[test]
    fn flip_payoffs_have_independent_deck_and_search_lock_requirements() {
        let mut needle = observation(NEEDLE_WORM);
        assert!(!access(&needle), "zero opponent Deck gives no mill payoff");
        needle.pile_sizes = vec![(0, Location::Deck, 0), (1, Location::Deck, 1)];
        assert!(access(&needle), "even one opponent card can be milled; our Deck is irrelevant");
        for count in [5, 26] {
            needle.pile_sizes[1].2 = count;
            assert!(access(&needle));
        }
        let mut cartaros = observation(CARTAROS);
        cartaros.pile_sizes[0].2 = 0;
        assert!(!access(&cartaros));
        cartaros.pile_sizes[0].2 = 26;
        for player in [0, 1] {
            let mut rai_oh = view(RAI_OH, Location::MonsterZone, 1);
            rai_oh.at.controller = player;
            rai_oh.attack = 0;
            cartaros.cards.push(rai_oh);
            assert!(!access(&cartaros));
            cartaros.cards.pop();
        }
        for obs in [&mut needle, &mut cartaros] {
            obs.cards.push(view(crate::knowledge::SKILL_DRAIN, Location::SpellTrapZone, 0));
            assert!(!access(obs));
        }
    }

    fn lock_choice(obs: &Observation, code: u32) -> Choice {
        let mut activation = choice(obs.cards.iter().find(|card| card.code == Some(code)).unwrap());
        activation.kind = ChoiceKind::Activate;
        activation.description = if code == FIRES_OF_DOOMSDAY { (code as u64) << 20 } else { 0 };
        activation
    }

    fn activation_permission(obs: &Observation, code: u32, other_choices: Vec<Choice>) -> Option<bool> {
        let cards = cards();
        let mut memory = Memory::default();
        let activation = lock_choice(obs, code);
        let mut choices = vec![activation.clone()];
        choices.extend(other_choices);
        let decision = prompt(choices);
        let turn = Turn { ctx: Ctx::new(obs, &cards), decision: &decision, memory: &mut memory };
        allow_activation(&turn, &activation)
    }

    #[test]
    fn own_draw_token_activation_preserves_visible_affordable_normal_summons() {
        for code in [SCAPEGOAT, FIRES_OF_DOOMSDAY] {
            let mut obs = observation(UNSEEN_OTHER);
            obs.phase = Some(Phase::Draw);
            obs.summon_used = false;
            obs.cards.push(view(code, Location::Hand, 0));
            obs.cards.push(view(UNSEEN_FUEL, Location::Hand, 1));
            assert_eq!(activation_permission(&obs, code, vec![]), Some(false));
            obs.summon_used = true;
            assert_eq!(activation_permission(&obs, code, vec![]), None);
            obs.summon_used = false;
            obs.cards.pop();
            assert_eq!(activation_permission(&obs, code, vec![]), None, "no known normal-summon fuel");
            for sequence in 1..5 { obs.cards.push(view(UNSEEN_OTHER, Location::MonsterZone, sequence)); }
            obs.cards.push(view(UNSEEN_FUEL, Location::Hand, 1));
            assert_eq!(activation_permission(&obs, code, vec![]), None, "no ordinary normal-summon space");
        }
    }

    #[test]
    fn exact_token_timing_veto_keeps_reactive_and_opponent_end_uses_available() {
        for code in [SCAPEGOAT, FIRES_OF_DOOMSDAY] {
            let mut obs = observation(UNSEEN_OTHER);
            obs.summon_used = false;
            obs.cards.push(view(code, Location::SpellTrapZone, 0));
            obs.cards.push(view(UNSEEN_FUEL, Location::Hand, 0));
            assert_eq!(activation_permission(&obs, code, vec![]), Some(false));
            obs.turn_player = Some(1);
            obs.phase = Some(Phase::End);
            assert_eq!(activation_permission(&obs, code, vec![]), None);
            obs.turn_player = Some(0);
            assert_eq!(activation_permission(&obs, code, vec![]), None, "our End cannot develop further");
            obs.phase = Some(Phase::Main1);
            obs.chain.push(ChainLink { code: UNSEEN_OTHER, controller: 1,
                source: CardRef { controller: 1, location: Location::SpellTrapZone, sequence: 0 }, targets: vec![] });
            assert_eq!(activation_permission(&obs, code, vec![]), None, "retain responses to opponents");
            obs.chain.clear();
            let cards = cards();
            let mut memory = Memory::default();
            let mut activation = lock_choice(&obs, code);
            activation.description += 1;
            let decision = prompt(vec![activation.clone()]);
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &decision, memory: &mut memory };
            assert_eq!(allow_activation(&turn, &activation), None, "unrelated effect slot");
        }
    }

    #[test]
    fn token_veto_uses_actual_summons_but_does_not_confuse_setting_with_summoning() {
        for code in [SCAPEGOAT, FIRES_OF_DOOMSDAY] {
            let mut obs = observation(UNSEEN_OTHER);
            obs.cards.push(view(code, Location::Hand, 0));
            let mut development = choice(&obs.cards[0]);
            for kind in [ChoiceKind::NormalSummon, ChoiceKind::SpecialSummon, ChoiceKind::ChangePosition] {
                development.kind = kind;
                assert_eq!(activation_permission(&obs, code, vec![development.clone()]), Some(false));
            }
            development.kind = ChoiceKind::SetMonster;
            assert_eq!(activation_permission(&obs, code, vec![development.clone()]), None,
                "Scapegoat/Fires forbid summons but still permit a Normal Set");
            development.kind = ChoiceKind::ChangePosition;
            obs.cards[0].position = Position::FACE_UP_DEFENSE;
            assert_eq!(activation_permission(&obs, code, vec![development.clone()]), None,
                "ordinary repositioning is not a Flip Summon");
        }
    }

    #[test]
    fn selected_token_activation_does_not_prove_a_surviving_summon_oath() {
        let obs = observation(TANNGNJOSTR);
        let cards = cards();
        let change = choice(&obs.cards[0]);
        let decision = prompt(vec![change.clone()]);
        for code in [SCAPEGOAT, FIRES_OF_DOOMSDAY] {
            let mut memory = Memory::default();
            memory.activated.push(code);
            let turn = Turn { ctx: Ctx::new(&obs, &cards), decision: &decision, memory: &mut memory };
            // Memory records the choice but not whether its activation was
            // negated. processor.cpp removes OATH effects in that case; effect
            // negation alone would retain them. Neither outcome is inferred.
            assert!(change_position_for_access(&turn, &change));
        }
    }

    #[test]
    fn all_position_access_sources_reject_public_fatal_attack_exposure() {
        for code in [TANNGNJOSTR, REMOTEN, CARTAROS, NEEDLE_WORM] {
            let mut obs = observation(code);
            obs.life_points[0] = 500;
            obs.pile_sizes.push((1, Location::Deck, 5));
            assert!(access(&obs), "no current enemy attacker");
            let mut enemy = view(UNSEEN_OTHER, Location::MonsterZone, 0);
            enemy.at.controller = 1;
            enemy.attack = 1800;
            obs.cards.push(enemy);
            assert!(!access(&obs), "{code}: 300 ATK into 1800 ATK at 500 LP");
            let mut defender = view(UNSEEN_OTHER, Location::MonsterZone, 1);
            defender.attack = 3000;
            obs.cards.push(defender);
            assert!(!access(&obs), "another friendly body cannot shield the exposed source");
            obs.cards[2].attack = 800;
            assert!(!access(&obs), "equal to remaining LP is also lethal");
            if matches!(code, TANNGNJOSTR | REMOTEN) {
                obs.life_points[0] = 501;
                assert!(access(&obs), "nonfatal exposure may be justified by access");
            }
        }
    }
}
