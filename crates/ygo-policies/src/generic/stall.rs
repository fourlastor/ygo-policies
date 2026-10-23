//! Battle stoppers when our side has no attacker of its own.
//!
//! A side that is not going to win its battles (a burn, deck-out or
//! alternate-win deck, or a hand with nothing to summon) loses only through
//! its Life Points: every attack it stops is time bought, and a turn-long
//! shield covers every battle of the turn it is used in. The shared staple
//! timing keeps these cards for attacks that cost a monster or a lot of Life
//! Points; with no attacker of our own, any damage is worth stopping.
use crate::agent::{Response, Turn};
use crate::cards::types;
use crate::model::{Choice, ChoiceKind, Phase};
use crate::staples::{THREATENING_ROAR, WABOKU};

/// The ATK a monster needs to count as an attacker of ours.
const ATTACKER: i32 = 1500;
const SWIFT_SCARECROW: u32 = 18964575;
const BATTLE_FADER: u32 = 19665973;
/// Turn-long battle shields: one is enough for a turn.
const SHIELDS: &[u32] = &[WABOKU, THREATENING_ROAR, SWIFT_SCARECROW, BATTLE_FADER];

pub const SUPPORTED_CODES: &[u32] = &[WABOKU, THREATENING_ROAR];

/// No attacker of ours: no face-up monster on our field, and no monster in
/// our hand we could Normal Summon without a Tribute, with 1500 ATK or more.
pub(super) fn stalling(t: &Turn) -> bool {
    let ctx = t.ctx;
    let on_field = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && c.attack >= ATTACKER);
    let in_hand = ctx.hand().iter().any(|c| {
        let data = ctx.view_data(c);
        data.is_monster() && !data.is_extra() && data.tributes() == 0 && data.attack >= ATTACKER
    });
    !on_field && !in_hand
}

/// With no attacker of ours, a Normal Monster that defends better than it
/// attacks is Set: face-up in Attack Position it only invites an attack
/// (0/2400 Gem-Knight Sapphire against an empty field). An Effect Monster
/// may need to be face-up to work (Fire Princess), and keeps the shared rule.
pub(super) fn summon_score(t: &Turn, choice: &Choice) -> Option<Option<f64>> {
    let data = t.ctx.data(choice.code()?);
    if data.tributes() != 0 || !data.is(types::NORMAL) || data.is(types::EFFECT)
        || data.defense <= data.attack || !stalling(t)
    {
        return None;
    }
    match choice.kind {
        ChoiceKind::NormalSummon => Some(None),
        ChoiceKind::SetMonster => Some(Some(data.defense as f64)),
        _ => None,
    }
}

pub(super) fn chain(t: &Turn, index: usize) -> Option<Response> {
    let ctx = t.ctx;
    let code = ctx.canonical(t.choice(index).code()?);
    if !SUPPORTED_CODES.contains(&code) || !stalling(t) {
        return None;
    }
    if t.memory.activated.iter().any(|k| SHIELDS.contains(&ctx.canonical(*k))) {
        return Some(Response::no());
    }
    match code {
        WABOKU => {
            let (attacker, target) = ctx.incoming_attack()?;
            let hurts = match target {
                None => attacker.attack > 0,
                Some(_) => ctx.attack_hurts(attacker, target),
            };
            hurts.then(|| Response::new(90.0))
        }
        THREATENING_ROAR => {
            let opening = !ctx.my_turn() && ctx.phase() == Some(Phase::BattleStart);
            let attackers = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up && c.position.attack && c.attack > 0);
            (opening && attackers).then(|| Response::new(85.0))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Memory;
    use crate::cards::{types, CardData, MemoryCards};
    use crate::ctx::Ctx;
    use crate::model::{CardRef, CardView, Choice, ChoiceKind, Decision, DecisionKind, Hint, Location, Member, Observation, Position};

    const THEIRS: u32 = 900_800_001;
    const OURS: u32 = 900_800_002;

    fn database() -> MemoryCards {
        let trap = |code| (code, CardData { code, kind: types::TRAP, ..CardData::default() });
        let monster = |code, attack| (code, CardData { code, kind: types::MONSTER | types::NORMAL, attack, defense: 1000, level: 4, ..CardData::default() });
        MemoryCards([trap(WABOKU), trap(THREATENING_ROAR), monster(THEIRS, 1000), monster(OURS, 1800)].into_iter().collect())
    }

    fn card(code: u32, controller: u8, location: Location, sequence: u32, attack: i32) -> CardView {
        CardView {
            at: CardRef { controller, location, sequence },
            code: Some(code),
            position: if location == Location::SpellTrapZone { Position::FACE_DOWN_DEFENSE } else { Position::FACE_UP_ATTACK },
            attack,
            defense: 1000,
            level: 4,
            can_attack: true,
            battles: 0,
            counters: 0,
            coin_effect: None, negated: false,
        }
    }

    /// Their 1000-ATK monster, on their turn; `attacking` declares a direct attack.
    fn observation(phase: Phase, attacking: bool, ours: Option<Location>) -> Observation {
        let theirs = card(THEIRS, 1, Location::MonsterZone, 0, 1000);
        let attacker = theirs.at;
        let mut cards = vec![theirs, card(WABOKU, 0, Location::SpellTrapZone, 0, 0), card(THREATENING_ROAR, 0, Location::SpellTrapZone, 1, 0)];
        if let Some(location) = ours {
            cards.push(card(OURS, 0, location, 0, 1800));
        }
        Observation {
            me: 0,
            turn: 2,
            turn_player: Some(1),
            phase: Some(phase),
            life_points: [8000; 2],
            battle_attacker: attacking.then_some(attacker),
            battle_target: None,
            cards,
            pile_sizes: vec![],
            chain: vec![],
            event_cards: vec![],
            summon_used: false,
            chain_known: true,
            can_attack_known: true,
            coin_toss: None,
        }
    }

    fn score(obs: &Observation, code: u32, memory: &mut Memory) -> Option<f64> {
        let db = database();
        let sequence = if code == WABOKU { 0 } else { 1 };
        let activate = Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member { at: CardRef { controller: 0, location: Location::SpellTrapZone, sequence }, code: Some(code), value: 0, required: false }),
            members: vec![],
            description: 0,
            place: None,
        };
        let pass = Choice { kind: ChoiceKind::Pass, card: None, members: vec![], description: 0, place: None };
        let prompt = Decision {
            kind: DecisionKind::Chain { forced: false, triggers: false },
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: None,
            choices: vec![activate, pass],
        };
        chain(&Turn { ctx: Ctx::new(obs, &db), decision: &prompt, memory }, 0).map(|response| response.score)
    }

    #[test]
    fn with_no_attacker_of_ours_any_damage_is_stopped_once_a_turn() {
        let attack = observation(Phase::BattleStep, true, None);
        // The shared timing lets a 1000-ATK direct attack through at 8000 LP.
        assert_eq!(score(&attack, WABOKU, &mut Memory::default()), Some(90.0));
        let opening = observation(Phase::BattleStart, false, None);
        assert_eq!(score(&opening, THREATENING_ROAR, &mut Memory::default()), Some(85.0));
        // One shield covers the turn.
        let mut memory = Memory::default();
        memory.activated.push(THREATENING_ROAR);
        assert!(score(&attack, WABOKU, &mut memory).is_some_and(|s| s <= 0.0));
    }

    #[test]
    fn an_attacker_of_ours_leaves_the_shared_timing_in_charge() {
        for ours in [Location::MonsterZone, Location::Hand] {
            let attack = observation(Phase::BattleStep, true, Some(ours));
            assert_eq!(score(&attack, WABOKU, &mut Memory::default()), None);
            let opening = observation(Phase::BattleStart, false, Some(ours));
            assert_eq!(score(&opening, THREATENING_ROAR, &mut Memory::default()), None);
        }
    }
}
