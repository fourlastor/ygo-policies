//! A deck-independent pilot for ordinary play and bounded engine rollouts.
//!
//! The policy sees no deck id or opponent policy. Printed card properties,
//! public board information, legal actions and the shared staple/card facts
//! are its entire input. Unknown effects are deliberately only a prior:
//! legal availability does not establish that activating an effect is good.
//! An engine search can override these choices after simulating the effect.

use crate::agent::{Response, Strategy, Turn};
use crate::cards::{effect_categories as cat, effect_events as event, effect_locations as range,
    effect_types as effect_type, types, EffectDescriptor};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, DecisionKind, Hint, Location, Member, Phase};
use crate::{staples, tactics};

#[derive(Clone, Default)]
pub struct Generic;

impl Generic {
    const ACCESS: u64 = cat::DRAW | cat::SEARCH;
    const DEVELOP: u64 = Self::ACCESS | cat::TO_HAND | cat::SPECIAL_SUMMON | cat::TOKEN;
    const REMOVE: u64 = cat::DESTROY | cat::REMOVE | cat::TO_DECK | cat::TO_GRAVE | cat::CONTROL;

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

    /// Match a legal effect's description instead of merging unrelated effects
    /// of a multi-effect card. Unannotated or dynamically created effects keep
    /// the ordinary unknown-effect fallback.
    fn offered_effects<'a>(t: &Turn<'a>, choice: &Choice) -> impl Iterator<Item = &'a EffectDescriptor> {
        let ctx = t.ctx;
        let code = choice.code().unwrap_or(0);
        let location = match choice.at().map(|at| at.location) {
            Some(Location::Hand) => range::HAND,
            Some(Location::MonsterZone) => range::MONSTER,
            Some(Location::SpellTrapZone) => range::SPELL_TRAP,
            Some(Location::Graveyard) => range::GRAVEYARD,
            Some(Location::Banished) => range::BANISHED,
            Some(Location::Extra) => range::EXTRA,
            _ => 0,
        };
        let sequence = choice.at().map_or(0, |at| at.sequence);
        let description = choice.description;
        let allowed_type = match t.decision.kind {
            DecisionKind::Chain { triggers: true, .. } => effect_type::TRIGGER | 0x20,
            DecisionKind::Idle | DecisionKind::Battle | DecisionKind::Chain { triggers: false, .. } => {
                effect_type::ACTIVATE | effect_type::IGNITION | effect_type::QUICK
            }
            _ => effect_type::ACTION,
        };
        ctx.db.effects(code).iter().filter(move |effect| {
            effect.effect_type & allowed_type != 0
                && effect.description == description
                && (effect.range == 0 || range::matches_at(effect.range, location, sequence))
        })
    }

    fn offered_categories(t: &Turn, choice: &Choice) -> u64 {
        Self::offered_effects(t, choice).fold(0, |categories, effect| categories | effect.category)
    }

    fn grave_utility(ctx: &Ctx, code: u32) -> f64 {
        ctx.db.effects(code).iter().filter_map(|effect| {
            if effect.category & (Self::DEVELOP | Self::REMOVE | cat::DAMAGE) == 0 {
                return None;
            }
            let from_grave = effect.range & range::GRAVEYARD != 0
                && effect.effect_type & effect_type::ACTION != 0;
            let on_arrival = effect.event == event::TO_GRAVE
                && effect.effect_type & effect_type::TRIGGER != 0;
            if !from_grave && !on_arrival {
                return None;
            }
            // This is option value, not a claim that opaque conditions are met.
            // A small body with a graveyard action is useful setup material.
            let value = if from_grave && effect.effect_type & effect_type::IGNITION != 0 {
                2600.0
            } else {
                1900.0
            };
            Some(value - if effect.has_cost { 350.0 } else { 0.0 })
        }).fold(0.0, f64::max)
    }

    fn normal_summon_reward(ctx: &Ctx, code: u32) -> f64 {
        let own_trigger = ctx.db.effects(code).iter().any(|effect| {
            effect.effect_type & effect_type::TRIGGER != 0
                && effect.event == event::SUMMON_SUCCESS
                && effect.category & (Self::DEVELOP | cat::EQUIP) != 0
        });
        let field_trigger = ctx.obs.cards.iter().filter(|card| {
            card.at.controller == ctx.me && card.at.location == Location::SpellTrapZone && card.position.face_up
        }).filter_map(|card| card.code).any(|source| ctx.db.effects(source).iter().any(|effect| {
            effect.effect_type & effect_type::TRIGGER != 0
                && effect.event == event::SUMMON_SUCCESS
                && range::normalize(effect.range) & range::SPELL_TRAP != 0
                && effect.category & Self::DEVELOP != 0
        }));
        if own_trigger { 1400.0 } else if field_trigger { 600.0 } else { 0.0 }
    }

    fn gain_value(ctx: &Ctx, code: u32, hint: Hint) -> f64 {
        let data = ctx.data(code);
        let mut value = Self::body_value(ctx, code) as f64;
        if matches!(hint, Hint::SpecialSummon | Hint::Summon)
            && ctx.my_turn() && ctx.phase() != Some(Phase::Main2)
        {
            // A body entering before battle is valuable for the attacks it can
            // make. Printed DEF alone must not outrank an attacker that clears
            // the opposing board. Resolution and materials remain search work.
            value += (data.attack.max(0) - data.attack.max(data.defense).max(0)) as f64;
            let threat = ctx.opp_best_attack();
            if threat > 0 && data.attack > threat { value += 800.0; }
        }
        if hint == Hint::AddToHand {
            let copies = ctx.hand().iter().filter(|card| card.code.is_some_and(|other| ctx.canonical(other) == ctx.canonical(code))).count();
            value -= 650.0 * copies as f64;
            // Fusion/Synchro/etc. monsters return to the Extra Deck instead
            // of becoming a hand resource when an effect says "add to hand".
            if data.is_extra() { value -= 2200.0; }
            if !data.is_extra() && data.tributes() == 0 {
                value += Self::normal_summon_reward(ctx, code) * 0.5;
            }
        }
        if hint == Hint::Equip && ctx.db.effects(code).iter().any(|effect| {
            range::normalize(effect.range) & range::SPELL_TRAP != 0
                && effect.effect_type & effect_type::ACTION != 0
                && effect.category & Self::DEVELOP != 0
        }) {
            // Monsters placed as equipment can have actions from the backrow;
            // their body stats do not describe that destination's usefulness.
            value += 2200.0;
        }
        value
    }

    fn setup_before_summon(&self, t: &Turn, index: usize) -> bool {
        let choice = t.choice(index);
        let Some(code) = choice.code() else { return false };
        let data = t.ctx.data(code);
        if choice.kind != ChoiceKind::Activate || !t.fresh(index) || t.own_top()
            || !data.is_spell() || !data.is(types::CONTINUOUS | types::FIELD)
            || choice.at().is_none_or(|at| at.controller != t.ctx.me)
            || t.view(choice).is_some_and(|view| view.at.location == Location::SpellTrapZone && view.position.face_up)
        {
            return false;
        }
        t.ctx.db.effects(code).iter().any(|effect| {
            range::normalize(effect.range) & range::SPELL_TRAP != 0
                && (effect.effect_type & effect_type::IGNITION != 0 && effect.category & Self::ACCESS != 0
                    && t.choices().any(|(i, summon)| t.fresh(i) && summon.kind == ChoiceKind::NormalSummon)
                    || effect.effect_type & effect_type::TRIGGER != 0 && effect.category & Self::DEVELOP != 0
                    && t.choices().any(|(i, summon)| t.fresh(i) && match effect.event {
                    event::SUMMON_SUCCESS => summon.kind == ChoiceKind::NormalSummon,
                    event::SPECIAL_SUMMON_SUCCESS => summon.kind == ChoiceKind::SpecialSummon,
                    // A position change may be a flip summon, but also may be
                    // only a battle-position change. Leave that to simulation.
                    _ => false,
                }))
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
        let categories = Self::offered_categories(t, choice);
        if (data.is_trap() || data.is(types::QUICKPLAY)) && categories & Self::ACCESS == 0 {
            return None;
        }
        let score = match at.location {
            Location::Hand if data.is_spell() => 40,
            Location::Hand if data.is_monster() => 35,
            Location::Graveyard | Location::Banished => 35,
            Location::MonsterZone => 30,
            Location::SpellTrapZone if data.is_spell() || categories & Self::ACCESS != 0 => 25,
            _ => return None,
        };
        Some(score + if categories & Self::ACCESS != 0 { 20 } else { 0 })
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
        // A summon-triggered engine has to be installed before the event it
        // rewards. The setup card's activation itself often has category zero.
        if let Some(index) = t.choices().find(|(i, _)| self.setup_before_summon(t, *i)).map(|(i, _)| i) {
            return t.pick(index);
        }
        // Searches/draws can expose the normal summon we actually want. The
        // legal offered effect, including its opaque conditions, is the gate.
        if t.has(ChoiceKind::NormalSummon) {
            if let Some(index) = t.choices().filter(|(i, choice)| {
                self.unknown_activation(t, *i).is_some()
                    && Self::offered_categories(t, choice) & Self::ACCESS != 0
            }).map(|(i, _)| i).next() {
                return t.pick(index);
            }
        }
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
        let reward = Self::normal_summon_reward(&t.ctx, code);
        if reward > 0.0 {
            return Some((choice.kind == ChoiceKind::NormalSummon).then_some(
                Self::body_value(&t.ctx, code) as f64 + reward
                    + if self.connects_synchro(t, code) { 1800.0 } else { 0.0 }
            ));
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
        let at = choice.at()?;
        if at.controller != t.ctx.me {
            return Some(Response::no());
        }
        let triggered = matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. });
        let categories = Self::offered_categories(t, choice);
        if t.own_top() {
            // Several independent optional triggers may share one event. A
            // known resource trigger is allowed to join ours; negation isn't.
            let productive_trigger = triggered && categories & Self::DEVELOP != 0
                && Self::offered_effects(t, choice).any(|e| e.effect_type & effect_type::TRIGGER != 0);
            return Some(if productive_trigger { Response::new(110.0) } else { Response::no() });
        }
        let counter = t.ctx.data(code).is(types::COUNTER) && !t.ctx.obs.chain.is_empty();
        if triggered || counter {
            return Some(Response::new(30.0));
        }
        let hostile_link = t.ctx.obs.chain.last().is_some_and(|link| link.controller == t.ctx.opp);
        let hostile_event = t.ctx.obs.event_cards.iter().any(|(at, _)| at.controller == t.ctx.opp);
        let hostile_attack = t.ctx.obs.battle_attacker.is_some_and(|at| at.controller == t.ctx.opp);
        if categories & (cat::NEGATE | cat::DISABLE_SUMMON | cat::DISABLE) != 0
            && (hostile_link || hostile_event)
        {
            return Some(Response::new(80.0));
        }
        let enemy_field = t.ctx.obs.cards.iter().any(|card| card.at.controller == t.ctx.opp && card.at.location.is_field());
        if categories & (Self::REMOVE | cat::TO_HAND) != 0 && enemy_field
            && (hostile_attack || hostile_event || hostile_link || t.ctx.my_turn())
        {
            return Some(Response::new(50.0));
        }
        if categories & Self::ACCESS != 0
            || categories & (cat::SPECIAL_SUMMON | cat::TOKEN) != 0
                && (t.ctx.my_turn() || hostile_attack || t.ctx.phase() == Some(Phase::End))
        {
            return Some(Response::new(35.0));
        }
        None
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let code = member.code?;
        let ctx = t.ctx;
        let hint = t.decision.hint;
        if member.at.controller != ctx.me {
            return None;
        }
        let worth = Self::body_value(&ctx, code) as f64;
        if matches!(hint, Hint::ToGraveyard | Hint::Discard | Hint::FusionMaterial | Hint::SynchroMaterial | Hint::Tribute | Hint::Release) {
            let grave = Self::grave_utility(&ctx, code);
            if grave > 0.0 || hint == Hint::ToGraveyard && member.at.location == Location::Deck {
                // Deck-to-grave is setup, while a field/hand cost also loses
                // its current body or hand option. Do not conflate the two.
                let loss = if member.at.location == Location::Deck { worth * 0.1 } else { worth };
                let boss = if ctx.data(code).is_extra() && member.at.location == Location::MonsterZone { 1200.0 } else { 0.0 };
                return Some(grave - loss - boss);
            }
        }
        if hint == Hint::Banish && member.at.location == Location::Graveyard {
            return Some(-worth - Self::grave_utility(&ctx, code));
        }
        if hint.is_gain() && ctx.data(code).is_monster() {
            let connection = if self.connects_synchro(t, code) { 1200.0 } else { 0.0 };
            // A search should prefer a playable body over a stranded tribute.
            let tribute_cost = if hint == Hint::AddToHand && ctx.data(code).tributes() > ctx.monsters(ctx.me).len() as u32 {
                1600.0
            } else {
                0.0
            };
            return Some(Self::gain_value(&ctx, code, hint) + connection - tribute_cost);
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
    use crate::cards::{CardData, CardDatabase, EffectProfiles, MemoryCards, ProfiledCards};
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

    fn profiled(cards: Vec<CardData>, effects: EffectProfiles) -> Arc<dyn CardDatabase> {
        Arc::new(ProfiledCards::new(Arc::new(MemoryCards(cards.into_iter().map(|card| (card.code, card)).collect())), effects))
    }

    fn on_field(code: u32, controller: u8, location: Location, position: Position) -> CardView {
        CardView {
            at: CardRef { controller, location, sequence: 0 }, code: Some(code), position,
            attack: 1800, defense: 1000, level: 4, can_attack: true,
            battles: 0, counters: 0, coin_effect: None,
        }
    }

    #[test]
    fn unseen_summon_reward_is_installed_before_the_normal_summon() {
        let setup = 900001001;
        let body = 900001002;
        let database = profiled(vec![
            CardData { code: setup, kind: types::SPELL | types::CONTINUOUS, ..CardData::default() },
            CardData { code: body, kind: types::MONSTER, attack: 1900, level: 4, ..CardData::default() },
        ], [(setup, vec![EffectDescriptor {
            category: cat::SEARCH | cat::TO_HAND, effect_type: effect_type::TRIGGER,
            range: range::SPELL_TRAP, event: event::SUMMON_SUCCESS, ..EffectDescriptor::default()
        }])].into());
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::NormalSummon, Some(body), Location::Hand),
            choice(ChoiceKind::Activate, Some(setup), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 1);
    }

    #[test]
    fn unrelated_setup_event_does_not_preempt_the_summon() {
        let setup = 900001011;
        let body = 900001012;
        let database = profiled(vec![
            CardData { code: setup, kind: types::SPELL | types::CONTINUOUS, ..CardData::default() },
            CardData { code: body, kind: types::MONSTER, attack: 1900, level: 4, ..CardData::default() },
        ], [(setup, vec![EffectDescriptor {
            category: cat::SEARCH, effect_type: effect_type::TRIGGER,
            range: range::SPELL_TRAP, event: event::SPECIAL_SUMMON_SUCCESS, ..EffectDescriptor::default()
        }])].into());
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::NormalSummon, Some(body), Location::Hand),
            choice(ChoiceKind::Activate, Some(setup), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 0);
    }

    #[test]
    fn graveyard_destination_and_banish_cost_have_opposite_preferences() {
        let inert = 900001021;
        let usable = 900001022;
        let database = profiled(vec![
            CardData { code: inert, kind: types::MONSTER | types::EFFECT, attack: 0, level: 1, ..CardData::default() },
            CardData { code: usable, kind: types::MONSTER | types::EFFECT, attack: 400, level: 1, ..CardData::default() },
        ], [(usable, vec![EffectDescriptor {
            category: cat::SPECIAL_SUMMON, effect_type: effect_type::IGNITION,
            range: range::GRAVEYARD, has_cost: true, ..EffectDescriptor::default()
        }])].into());
        for (hint, location, expected) in [
            (Hint::ToGraveyard, Location::Deck, 1),
            (Hint::Discard, Location::Hand, 1),
            (Hint::Banish, Location::Graveyard, 0),
        ] {
            let mut pilot = Agent::seeded(Generic, database.clone(), 1);
            let mut prompt = decision(DecisionKind::SelectCards, [inert, usable].map(|code| {
                let mut candidate = choice(ChoiceKind::Cards, Some(code), location);
                candidate.members.push(candidate.card.take().unwrap());
                candidate
            }).to_vec());
            prompt.hint = hint;
            prompt.minimum = 1;
            prompt.maximum = 1;
            assert_eq!(pilot.choose(&observation(), &prompt), expected, "{hint:?}");
        }
    }

    #[test]
    fn unfamiliar_set_removal_answers_an_attack_but_not_an_empty_window() {
        let trap = 900001031;
        let database = profiled(vec![CardData { code: trap, kind: types::TRAP, ..CardData::default() }],
            [(trap, vec![EffectDescriptor { category: cat::DESTROY,
                effect_type: effect_type::ACTIVATE, ..EffectDescriptor::default() }])].into());
        let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, vec![
            choice(ChoiceKind::Activate, Some(trap), Location::SpellTrapZone),
            choice(ChoiceKind::Pass, None, Location::Other),
        ]);
        for attack in [false, true] {
            let mut pilot = Agent::seeded(Generic, database.clone(), 1);
            let mut obs = observation();
            obs.turn_player = Some(1);
            obs.cards.push(on_field(trap, 0, Location::SpellTrapZone, Position::FACE_DOWN_DEFENSE));
            obs.cards.push(on_field(900001032, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
            if attack { obs.battle_attacker = Some(obs.cards[1].at); }
            assert_eq!(pilot.choose(&obs, &prompt), usize::from(!attack));
        }
    }

    #[test]
    fn one_cards_other_effect_does_not_supply_the_offered_category() {
        let trap = 900001041;
        let database = profiled(vec![CardData { code: trap, kind: types::TRAP, ..CardData::default() }],
            [(trap, vec![EffectDescriptor { category: cat::DESTROY,
                effect_type: effect_type::ACTIVATE, description: 55, ..EffectDescriptor::default() }])].into());
        let mut pilot = Agent::seeded(Generic, database, 1);
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.cards.push(on_field(trap, 0, Location::SpellTrapZone, Position::FACE_DOWN_DEFENSE));
        obs.cards.push(on_field(900001042, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
        obs.battle_attacker = Some(obs.cards[1].at);
        let prompt = decision(DecisionKind::Chain { forced: false, triggers: false }, vec![
            choice(ChoiceKind::Activate, Some(trap), Location::SpellTrapZone),
            choice(ChoiceKind::Pass, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&obs, &prompt), 1);
    }

    #[test]
    fn known_resource_trigger_can_join_our_chain_but_negation_cannot() {
        let card = 900001051;
        for (category, effect_type, expected) in [
            (cat::DRAW, effect_type::TRIGGER, 0),
            (cat::NEGATE, effect_type::QUICK, 1),
        ] {
            let database = profiled(vec![CardData { code: card, kind: types::MONSTER | types::EFFECT, ..CardData::default() }],
                [(card, vec![EffectDescriptor { category, effect_type, ..EffectDescriptor::default() }])].into());
            let mut pilot = Agent::seeded(Generic, database, 1);
            let mut obs = observation();
            obs.chain.push(ChainLink { code: 900001052, controller: 0,
                source: CardRef { controller: 0, location: Location::MonsterZone, sequence: 1 }, targets: vec![] });
            let prompt = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
                choice(ChoiceKind::Activate, Some(card), Location::MonsterZone),
                choice(ChoiceKind::Pass, None, Location::Other),
            ]);
            assert_eq!(pilot.choose(&obs, &prompt), expected);
        }
    }

    #[test]
    fn resource_summon_triggers_take_priority_over_a_defensive_set() {
        let monster = 900003001;
        let field = 900003002;
        for source in [monster, field] {
            let database = profiled(vec![
                CardData { code: monster, kind: types::MONSTER | types::EFFECT, attack: 1300, defense: 1000, level: 4, ..CardData::default() },
                CardData { code: field, kind: types::SPELL | types::FIELD, ..CardData::default() },
            ], [(source, vec![EffectDescriptor {
                category: cat::SEARCH, event: event::SUMMON_SUCCESS, effect_type: effect_type::TRIGGER,
                range: if source == field { range::FIELD } else { 0 }, ..EffectDescriptor::default()
            }])].into());
            let mut pilot = Agent::seeded(Generic, database, 1);
            let mut obs = observation();
            obs.cards.push(on_field(900003003, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
            if source == field {
                let mut view = on_field(field, 0, Location::SpellTrapZone, Position::FACE_UP_ATTACK);
                view.at.sequence = 5;
                obs.cards.push(view);
            }
            let prompt = decision(DecisionKind::Idle, vec![
                choice(ChoiceKind::SetMonster, Some(monster), Location::Hand),
                choice(ChoiceKind::NormalSummon, Some(monster), Location::Hand),
                choice(ChoiceKind::EndTurn, None, Location::Other),
            ]);
            assert_eq!(pilot.choose(&obs, &prompt), 1, "reward source {source}");
        }
    }

    #[test]
    fn field_search_engine_is_installed_before_spending_normal_summon() {
        let field = 900003011;
        let monster = 900003012;
        let database = profiled(vec![
            CardData { code: field, kind: types::SPELL | types::FIELD, ..CardData::default() },
            CardData { code: monster, kind: types::MONSTER, attack: 1800, level: 4, ..CardData::default() },
        ], [(field, vec![EffectDescriptor {
            category: cat::SEARCH, effect_type: effect_type::IGNITION, range: range::FIELD, ..EffectDescriptor::default()
        }])].into());
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::NormalSummon, Some(monster), Location::Hand),
            choice(ChoiceKind::Activate, Some(field), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 1);
    }

    #[test]
    fn same_description_on_trigger_and_ignition_does_not_merge_categories() {
        let source = 900003021;
        let monster = 900003022;
        let database = profiled(vec![
            CardData { code: source, kind: types::MONSTER | types::EFFECT, level: 2, ..CardData::default() },
            CardData { code: monster, kind: types::MONSTER, attack: 1800, level: 4, ..CardData::default() },
        ], [(source, vec![
            EffectDescriptor { category: cat::SEARCH, effect_type: effect_type::TRIGGER, event: event::SUMMON_SUCCESS, description: 77, ..EffectDescriptor::default() },
            EffectDescriptor { category: 0, effect_type: effect_type::IGNITION, range: range::MONSTER, description: 77, ..EffectDescriptor::default() },
        ])].into());
        let mut pilot = Agent::seeded(Generic, database, 1);
        let mut activation = choice(ChoiceKind::Activate, Some(source), Location::MonsterZone);
        activation.description = 77;
        let prompt = decision(DecisionKind::Idle, vec![
            activation,
            choice(ChoiceKind::NormalSummon, Some(monster), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&observation(), &prompt), 1);
    }

    #[test]
    fn gain_scoring_values_destination_and_marginal_hand_resources() {
        let weak = 900003031;
        let strong = 900003032;
        let extra = 900003033;
        let equip = 900003034;
        let database = profiled(vec![
            CardData { code: weak, kind: types::MONSTER, attack: 1400, defense: 2600, level: 4, ..CardData::default() },
            CardData { code: strong, kind: types::MONSTER, attack: 2500, defense: 1300, level: 4, ..CardData::default() },
            CardData { code: extra, kind: types::MONSTER | types::FUSION, attack: 2800, defense: 2200, level: 7, ..CardData::default() },
            CardData { code: equip, kind: types::MONSTER | types::EFFECT, attack: 500, defense: 500, level: 2, ..CardData::default() },
        ], [(equip, vec![EffectDescriptor {
            category: cat::SPECIAL_SUMMON, effect_type: effect_type::IGNITION, range: range::SPELL_TRAP, ..EffectDescriptor::default()
        }])].into());
        for (hint, candidates, in_hand, expected) in [
            (Hint::SpecialSummon, [weak, strong], None, 1),
            (Hint::AddToHand, [weak, strong], Some(weak), 1),
            (Hint::AddToHand, [extra, strong], None, 1),
            (Hint::Equip, [strong, equip], None, 1),
        ] {
            let mut pilot = Agent::seeded(Generic, database.clone(), 1);
            let mut obs = observation();
            obs.cards.push(on_field(900003039, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
            if let Some(code) = in_hand { obs.cards.push(on_field(code, 0, Location::Hand, Position::FACE_DOWN_DEFENSE)); }
            let mut prompt = decision(DecisionKind::SelectCards, candidates.map(|code| {
                let mut candidate = choice(ChoiceKind::Cards, Some(code), Location::Graveyard);
                candidate.members.push(candidate.card.take().unwrap());
                candidate
            }).to_vec());
            prompt.hint = hint;
            prompt.minimum = 1;
            prompt.maximum = 1;
            assert_eq!(pilot.choose(&obs, &prompt), expected, "{hint:?}");
        }
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
