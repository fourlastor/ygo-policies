//! A deck-independent pilot for ordinary play and bounded engine rollouts.
//!
//! The policy sees no deck id or opponent policy. Printed card properties,
//! public board information, legal actions and the shared staple/card facts
//! are its entire input. Unknown effects are deliberately only a prior:
//! legal availability does not establish that activating an effect is good.
//! An engine search can override these choices after simulating the effect.

use crate::agent::{Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, DecisionKind, Hint, Location, Member};
use crate::{staples, tactics};

#[derive(Clone, Default)]
pub struct Generic;

impl Generic {
    /// Leave recognized staples to their existing timing/targeting rules.
    fn shared_effect(code: u32) -> bool {
        staples::value(code).is_some()
            || matches!(code, staples::SWORDS_OF_REVEALING_LIGHT | staples::BRIONAC | staples::SCRAP_DRAGON)
    }

    fn body_value(ctx: &Ctx, code: u32) -> i32 {
        let data = ctx.data(code);
        data.attack.max(data.defense).max(0)
            + if data.is(types::EFFECT) { 350 } else { 0 }
            + if data.is_tuner() { 200 } else { 0 }
    }

    fn connects_synchro(&self, t: &Turn, code: u32) -> bool {
        let ctx = t.ctx;
        let data = ctx.data(code);
        ctx.monsters(ctx.me).iter().any(|other| {
            other.position.face_up
                && ctx.view_data(other).is_tuner() != data.is_tuner()
                && tactics::synchro_worth(self, &ctx, other.level + data.level).is_some()
        })
    }

    fn unknown_activation(&self, t: &Turn, index: usize) -> Option<i32> {
        let choice = t.choice(index);
        if choice.kind != ChoiceKind::Activate || !t.fresh(index) || t.own_top() {
            return None;
        }
        let at = choice.at()?;
        let code = t.ctx.canonical(choice.code()?);
        if at.controller != t.ctx.me || Self::shared_effect(code) || t.ctx.wasted(code) {
            return None;
        }
        let data = t.ctx.data(code);
        if at.location == Location::MonsterZone && t.ctx.effects_drained() {
            return None;
        }
        // Reactive cards are left to their engine-gated response windows.
        // A legal unknown Trap in Main Phase may consume a defense needlessly.
        if data.is_trap() || data.is(types::QUICKPLAY) {
            return None;
        }
        let score = match at.location {
            Location::Hand if data.is_spell() => 40,
            Location::Hand if data.is_monster() => 35,
            Location::Graveyard | Location::Banished => 35,
            Location::MonsterZone => 30,
            Location::SpellTrapZone if data.is_spell() => 25,
            _ => return None,
        };
        Some(score)
    }
}

impl Strategy for Generic {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        if staples::value(code).is_some() {
            return None;
        }
        let data = ctx.data(code);
        data.is_monster().then(|| Self::body_value(ctx, code))
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        // Many free hand summons need an empty field. Use that opportunity
        // before spending the normal summon; the engine checks the conditions.
        if t.ctx.monsters(t.ctx.me).is_empty() {
            let choice = t.choices()
                .filter(|(i, c)| t.fresh(*i) && c.kind == ChoiceKind::SpecialSummon)
                .filter(|(_, c)| c.at().is_some_and(|a| a.location == Location::Hand))
                .max_by_key(|(_, c)| c.code().map_or(0, |code| Self::body_value(&t.ctx, code)))
                .map(|(i, _)| i);
            if let Some(index) = choice {
                return t.pick(index);
            }
        }
        None
    }

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        // Covers unseen cards without routing to a specialist. The shared
        // agent's per-turn action guard bounds repeatable/cyclic effects.
        let index = t.choices()
            .filter_map(|(index, _)| self.unknown_activation(t, index).map(|score| (score, index)))
            .max_by_key(|(score, index)| (*score, std::cmp::Reverse(*index)))
            .map(|(_, index)| index)?;
        t.pick(index)
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let code = choice.code()?;
        let data = t.ctx.data(code);
        if data.tributes() as usize > t.ctx.monsters(t.ctx.me).len() {
            // The engine has already offered this legal action. A reduced or
            // alternative summon procedure can require fewer tributes than
            // printed Level suggests; do not veto it for imagined missing
            // monsters. Keep the shared attack/defense and Flip preferences.
            let flip = data.is(types::FLIP);
            let attacker = data.attack >= 1600 && data.defense <= 1200;
            let face_up_ok = data.attack >= t.ctx.opp_best_attack()
                || t.ctx.monsters(t.ctx.opp).is_empty()
                || attacker;
            return match choice.kind {
                ChoiceKind::NormalSummon => Some((face_up_ok && !flip)
                    .then_some(Self::body_value(&t.ctx, code) as f64)),
                ChoiceKind::SetMonster => Some((!face_up_ok || flip)
                    .then_some(data.defense as f64 - 500.0 + if flip { 900.0 } else { 0.0 })),
                _ => None,
            };
        }
        if data.tributes() != 0 || data.is(types::FLIP) {
            return None;
        }
        if self.connects_synchro(t, code) {
            return Some((choice.kind == ChoiceKind::NormalSummon)
                .then_some(Self::body_value(&t.ctx, code) as f64 + 1800.0));
        }
        None
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let choice = t.choice(index);
        let code = t.ctx.canonical(choice.code()?);
        if Self::shared_effect(code) {
            return None;
        }
        if let Some(response) = staples::chain(self, t, index) {
            return Some(response);
        }
        if t.own_top() {
            return Some(Response::no());
        }
        let at = choice.at()?;
        if at.controller != t.ctx.me {
            return Some(Response::no());
        }
        let triggered = matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. });
        let counter = t.ctx.data(code).is(types::COUNTER) && !t.ctx.obs.chain.is_empty();
        if triggered || counter {
            Some(Response::new(30.0))
        } else {
            None
        }
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let code = member.code?;
        let ctx = t.ctx;
        let hint = t.decision.hint;
        if member.at.controller != ctx.me {
            return None;
        }
        let worth = Self::body_value(&ctx, code) as f64;
        if hint.is_gain() && ctx.data(code).is_monster() {
            let connection = if self.connects_synchro(t, code) { 1200.0 } else { 0.0 };
            // A search should prefer a playable body over a stranded tribute.
            let tribute_cost = if hint == Hint::AddToHand && ctx.data(code).tributes() > ctx.monsters(ctx.me).len() as u32 {
                1600.0
            } else {
                0.0
            };
            return Some(worth + connection - tribute_cost);
        }
        if hint == Hint::SynchroMaterial {
            // Preserve an existing boss when smaller material can make the
            // same legal summon; feasibility remains the engine's decision.
            return Some(-worth - if ctx.data(code).is_extra() { 1200.0 } else { 0.0 });
        }
        if hint == Hint::Target
            && t.memory.last_activated.is_some_and(|source| ctx.data(source).is(types::EQUIP))
        {
            return Some(worth);
        }
        if hint == Hint::Target && member.at.location == Location::Graveyard {
            let all_grave = t.choices().flat_map(|(_, c)| c.card.iter().chain(c.members.iter()))
                .all(|m| m.at.location == Location::Graveyard);
            if all_grave {
                return Some(worth);
            }
        }
        None
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        tactics::attack_directly(t)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::agent::{Agent, Policy};
    use crate::cards::{CardData, MemoryCards};
    use crate::model::*;

    fn observation() -> Observation {
        Observation {
            me: 0, turn: 2, turn_player: Some(0), phase: Some(Phase::Main1),
            life_points: [8000, 8000], cards: vec![], pile_sizes: vec![], chain: vec![],
            battle_attacker: None, battle_target: None, event_cards: vec![], summon_used: false,
            chain_known: true, can_attack_known: true, coin_toss: None,
        }
    }

    fn choice(kind: ChoiceKind, code: Option<u32>, location: Location) -> Choice {
        Choice {
            kind,
            card: code.map(|code| Member {
                at: CardRef { controller: 0, location, sequence: 0 },
                code: Some(code), value: 0, required: false,
            }),
            members: vec![], description: 0, place: None,
        }
    }

    fn decision(kind: DecisionKind, choices: Vec<Choice>) -> Decision {
        Decision { kind, hint: Hint::None, minimum: 0, maximum: 0, selected: vec![], subject: None, choices }
    }

    #[test]
    fn unseen_spell_is_used_and_repeated_actions_are_bounded() {
        let database = Arc::new(MemoryCards([(900000001, CardData { code: 900000001, kind: types::SPELL, ..CardData::default() })].into()));
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::Activate, Some(900000001), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        for _ in 0..3 {
            assert_eq!(pilot.choose(&observation(), &prompt), 0);
        }
        assert_eq!(pilot.choose(&observation(), &prompt), 1);
    }

    #[test]
    fn unknown_optional_trigger_does_not_answer_our_own_link() {
        let mut pilot = Agent::seeded(Generic, Arc::new(MemoryCards::default()), 1);
        let mut obs = observation();
        obs.chain.push(ChainLink {
            code: 900000002, controller: 0,
            source: CardRef { controller: 0, location: Location::MonsterZone, sequence: 0 }, targets: vec![],
        });
        let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, vec![
            choice(ChoiceKind::Activate, Some(900000003), Location::MonsterZone),
            choice(ChoiceKind::Pass, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&obs, &prompt), 1);
    }

    #[test]
    fn unknown_hand_trigger_is_allowed_in_trigger_window() {
        let mut pilot = Agent::seeded(Generic, Arc::new(MemoryCards::default()), 1);
        let prompt = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
            choice(ChoiceKind::Activate, Some(900000004), Location::Hand),
            choice(ChoiceKind::Pass, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 0);
    }

    #[test]
    fn rejected_shared_wipe_is_not_activated_as_an_unknown_spell() {
        let database = Arc::new(MemoryCards([(staples::DARK_HOLE, CardData {
            code: staples::DARK_HOLE, kind: types::SPELL, ..CardData::default()
        })].into()));
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::Activate, Some(staples::DARK_HOLE), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 1);
    }

    #[test]
    fn registry_creates_generic_without_a_benchmark_deck() {
        let database = Arc::new(MemoryCards::default());
        let mut pilot = crate::registry::create_seeded("generic", database, 5).unwrap();
        let prompt = decision(DecisionKind::Idle, vec![choice(ChoiceKind::EndTurn, None, Location::Other)]);
        assert_eq!(pilot.choose(&observation(), &prompt), 0);
        assert!(pilot.fork().is_some());
        assert!(!crate::registry::POLICIES.iter().any(|entry| entry.id == "generic"));
    }

    #[test]
    fn legal_unseen_high_level_summons_do_not_require_imagined_tributes() {
        // Synthetic passcodes: this must work from legal actions and printed
        // properties, without recognizing a deck or a reduced-tribute card.
        for (code, level, attack, defense, extra_kind, threat, expected) in [
            (900000101, 6, 2000, 800, 0, 1200, ChoiceKind::NormalSummon),
            (900000102, 8, 1000, 2500, 0, 2200, ChoiceKind::SetMonster),
            (900000103, 7, 2500, 1000, types::FLIP, 1200, ChoiceKind::SetMonster),
        ] {
            let database = Arc::new(MemoryCards([(code, CardData {
                code, kind: types::MONSTER | types::EFFECT | extra_kind,
                attack, defense, level, ..CardData::default()
            })].into()));
            let mut pilot = Agent::seeded(Generic, database, 1);
            let mut obs = observation();
            obs.cards.push(CardView {
                at: CardRef { controller: 1, location: Location::MonsterZone, sequence: 0 },
                code: Some(900000199), position: Position::FACE_UP_ATTACK,
                attack: threat, defense: 0, level: 4, can_attack: true,
                battles: 0, counters: 0, coin_effect: None,
            });
            let prompt = decision(DecisionKind::Idle, vec![
                choice(ChoiceKind::EndTurn, None, Location::Other),
                choice(ChoiceKind::NormalSummon, Some(code), Location::Hand),
                choice(ChoiceKind::SetMonster, Some(code), Location::Hand),
            ]);
            let chosen = pilot.choose(&obs, &prompt);
            assert_eq!(prompt.choices[chosen].kind, expected, "unseen card {code}");
        }
    }
}
