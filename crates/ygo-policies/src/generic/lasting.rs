//! Cards that work only while they are face-up: attack locks, Skill Drain,
//! the Pyramid and Morphtronic engines, and Ojama Trio's tokens.
//!
//! Set, they do nothing, and the unknown-effect fallback never activates a
//! Set card. The specialists that play them turn them face-up at the first
//! chance; so does this, under each card's own condition. These are
//! properties of the cards, not of any deck.
use super::stall;
use crate::agent::{Hostile, Response, Turn};
use crate::cards::types;
use crate::model::{ChoiceKind, Location};
use crate::staples::{GRAVITY_BIND, LEVEL_LIMIT_AREA_B, MESSENGER_OF_PEACE, MORPHTRONIC_BIND, NIGHTMARES_STEELCAGE, SWORDS_OF_REVEALING_LIGHT};

const SKILL_DRAIN: u32 = 82732705;
const PYRAMID_OF_LIGHT: u32 = 53569894;
const OJAMA_TRIO: u32 = 29843091;
const MORPHTRONIC: u16 = 0x26;
/// Locks that stop both players' attacks: only for a side with no attacker.
const BOTH_SIDES: &[u32] = &[GRAVITY_BIND, LEVEL_LIMIT_AREA_B, MESSENGER_OF_PEACE, NIGHTMARES_STEELCAGE];

pub const SUPPORTED_CODES: &[u32] = &[
    GRAVITY_BIND,
    LEVEL_LIMIT_AREA_B,
    MESSENGER_OF_PEACE,
    NIGHTMARES_STEELCAGE,
    SWORDS_OF_REVEALING_LIGHT,
    MORPHTRONIC_BIND,
    SKILL_DRAIN,
    PYRAMID_OF_LIGHT,
    OJAMA_TRIO,
];

/// Whether to turn this card face-up now, `None` for other cards.
fn wanted(t: &Turn, index: usize) -> Option<bool> {
    let ctx = t.ctx;
    let choice = t.choice(index);
    let code = ctx.canonical(choice.code()?);
    if !SUPPORTED_CODES.contains(&code) {
        return None;
    }
    // Already working: a second copy adds nothing.
    let up = |code: u32| ctx.spell_traps(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, code));
    if up(code) {
        return Some(false);
    }
    let theirs = ctx.monsters(ctx.opp);
    Some(match code {
        // These stop our attacks too; each covers monsters the others do not
        // (Level 4 and up, 1500 ATK and up).
        _ if BOTH_SIDES.contains(&code) => stall::stalling(t) && (code != MESSENGER_OF_PEACE || ctx.my_lp() > 2000),
        // Their monsters only: whenever they have one, or we have no attacker.
        SWORDS_OF_REVEALING_LIGHT => !theirs.is_empty() && (stall::stalling(t) || ctx.opp_best_attack() > ctx.my_best_attack()),
        // Our own Set monsters are known to us.
        MORPHTRONIC_BIND => ctx.monsters(ctx.me).iter().any(|c| ctx.view_data(c).in_set(MORPHTRONIC)),
        PYRAMID_OF_LIGHT => true,
        OJAMA_TRIO => stall::stalling(t) && ctx.free_monster_zones(ctx.opp) >= 3,
        // It negates every face-up monster's effect: never over one of ours.
        SKILL_DRAIN => {
            let ours = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is(types::EFFECT));
            let answered = matches!(t.hostile_top(), Hostile::Link(link) if link.source.location == Location::MonsterZone);
            let theirs_work = theirs.iter().any(|c| c.position.face_up && ctx.view_data(c).is(types::EFFECT));
            ctx.my_lp() > 2000 && !ours && (answered || theirs_work || stall::stalling(t))
        }
        _ => false,
    })
}

pub(super) fn chain(t: &Turn, index: usize) -> Option<Response> {
    Some(if wanted(t, index)? { Response::new(88.0) } else { Response::no() })
}

/// In our Main Phase, before anything else is developed.
pub(super) fn main_phase(t: &mut Turn) -> Option<usize> {
    let index = t.choices()
        .find(|(index, choice)| {
            choice.kind == ChoiceKind::Activate && t.fresh(*index) && wanted(t, *index) == Some(true)
                && choice.code().is_some_and(|code| super::alternate::allow_staple(t, code))
        })
        .map(|(index, _)| index)?;
    t.pick(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{CardData, MemoryCards};
    use crate::ctx::Ctx;
    use crate::model::{CardRef, CardView, Choice, Decision, DecisionKind, Hint, Member, Observation, Phase, Position};

    const THEIRS: u32 = 900_810_001;
    const ATTACKER: u32 = 900_810_002;
    const DEFENDER: u32 = 900_810_003;

    fn database() -> MemoryCards {
        let card = |code, kind, attack| (code, CardData { code, kind, attack, defense: 2000, level: 4, ..CardData::default() });
        MemoryCards([
            card(GRAVITY_BIND, types::TRAP | types::CONTINUOUS, 0),
            card(SKILL_DRAIN, types::TRAP | types::CONTINUOUS, 0),
            card(THEIRS, types::MONSTER | types::EFFECT, 1900),
            card(ATTACKER, types::MONSTER | types::EFFECT, 1800),
            card(DEFENDER, types::MONSTER | types::NORMAL, 0),
        ].into_iter().collect())
    }

    fn view(code: u32, controller: u8, location: Location, sequence: u32, position: Position) -> CardView {
        let attack = database().0[&code].attack;
        CardView { at: CardRef { controller, location, sequence }, code: Some(code), position, attack, defense: 2000,
            level: 4, can_attack: true, battles: 0, counters: 0, coin_effect: None }
    }

    /// Their turn, their Effect Monster face-up; ours is `ours`, face-up.
    fn score(code: u32, ours: u32) -> Option<f64> {
        let db = database();
        let obs = Observation {
            me: 0, turn: 3, turn_player: Some(1), phase: Some(Phase::Main1), life_points: [8000; 2],
            cards: vec![
                view(THEIRS, 1, Location::MonsterZone, 0, Position::FACE_UP_ATTACK),
                view(ours, 0, Location::MonsterZone, 0, Position::FACE_UP_ATTACK),
                view(code, 0, Location::SpellTrapZone, 0, Position::FACE_DOWN_DEFENSE),
            ],
            pile_sizes: vec![], chain: vec![], battle_attacker: None, battle_target: None, event_cards: vec![],
            summon_used: false, chain_known: true, can_attack_known: true, coin_toss: None,
        };
        let activate = Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member { at: CardRef { controller: 0, location: Location::SpellTrapZone, sequence: 0 }, code: Some(code), value: 0, required: false }),
            members: vec![], description: 0, place: None,
        };
        let prompt = Decision { kind: DecisionKind::Chain { forced: false, triggers: false }, hint: Hint::None, minimum: 0,
            maximum: 0, selected: vec![], subject: None, choices: vec![activate] };
        let mut memory = Memory::default();
        chain(&Turn { ctx: Ctx::new(&obs, &db), decision: &prompt, memory: &mut memory }, 0).map(|response| response.score)
    }

    #[test]
    fn a_two_sided_lock_waits_for_a_side_with_no_attacker_of_its_own() {
        assert_eq!(score(GRAVITY_BIND, DEFENDER), Some(88.0));
        assert!(score(GRAVITY_BIND, ATTACKER).is_some_and(|score| score <= 0.0));
    }

    #[test]
    fn skill_drain_is_never_turned_over_our_own_effect_monster() {
        assert_eq!(score(SKILL_DRAIN, DEFENDER), Some(88.0));
        assert!(score(SKILL_DRAIN, ATTACKER).is_some_and(|score| score <= 0.0));
    }
}
