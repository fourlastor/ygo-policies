//! A deck-independent pilot for ordinary play and bounded engine rollouts.
//!
//! The policy sees no deck id or opponent policy. Printed card properties,
//! public board information, legal actions and the shared staple/card facts
//! are its entire input. Unknown effects are deliberately only a prior:
//! legal availability does not establish that activating an effect is good.
//! An engine search can override these choices after simulating the effect.

pub mod alternate;
mod combat;
mod conversion;
pub mod knowledge;
mod materials;
pub mod requirements;
mod transactions;
pub use conversion::SUPPORTED_CODES as CONVERSION_SUPPORTED_CODES;
pub use combat::SUPPORTED_CODES as COMBAT_SUPPORTED_CODES;
pub use materials::SUPPORTED_CODES as MATERIAL_SUPPORTED_CODES;
pub use transactions::SUPPORTED_CODES as TRANSACTION_SUPPORTED_CODES;

use crate::agent::{Outcome, Response, Strategy, Turn};
use crate::cards::{effect_categories as cat, effect_events as event, effect_locations as range,
    effect_types as effect_type, types, EffectDescriptor};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, DecisionKind, Hint, Location, Member, Phase, Position};
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
                && effect.effect_type & effect_type::TRIGGER != 0
                && requirements::arrival_requirement(ctx.canonical(code), effect).is_none();
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
        let counter_trigger = knowledge::profile(ctx.canonical(code)).counter_engine.is_some_and(|engine| {
            matches!(engine.normal_summon_gain, knowledge::CounterGain::FaceUpSetOnBothFields { .. })
        });
        let own_trigger = ctx.db.effects(code).iter().any(|effect| {
            effect.effect_type & effect_type::TRIGGER != 0
                && effect.event == event::SUMMON_SUCCESS
                // The summoned card is now in a main Monster Zone. A hand
                // trigger reacting to somebody else's summon must stay in
                // hand; summoning that card cannot earn its effect's reward.
                && (effect.range == 0 || range::matches_at(effect.range, range::MONSTER, 0))
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
        if own_trigger || counter_trigger { 1400.0 } else if field_trigger { 600.0 } else { 0.0 }
    }

    fn shield_useful(t: &Turn, choice: &Choice) -> bool {
        let ctx = t.ctx;
        let Some(code) = choice.code().map(|code| ctx.canonical(code)) else { return false };
        if !ctx.data(code).is_spell() && !ctx.data(code).is_trap() { return false; }
        if t.view(choice).is_some_and(|card| card.at.location.is_field() && card.position.face_up) { return false; }
        if ctx.data(code).is_trap() && ctx.obs.cards.iter().any(|card| {
            card.at.location.is_field() && card.position.face_up
                && card.code.is_some_and(|code| knowledge::profile(ctx.canonical(code)).negates_field_traps)
                && (card.at.location != Location::MonsterZone || !ctx.effects_drained())
        }) { return false; }
        match knowledge::profile(code).damage_shield {
            Some(knowledge::DamageShield::ControllerWhileControlsMonster) => !ctx.monsters(ctx.me).is_empty(),
            Some(knowledge::DamageShield::BattleInvolvingRace { race }) => ctx.monsters(ctx.me).iter().any(|card| {
                card.position.face_up && ctx.view_data(card).race & race != 0
            }),
            None => false,
        }
    }

    /// Exact source facts constrain a legal transaction without guessing its
    /// cost from printed Level or treating unrelated modes as interchangeable.
    fn known_activation_allowed(t: &Turn, choice: &Choice) -> bool {
        if transactions::allow_activation(t, choice) == Some(false)
            || combat::allow_activation(t, choice) == Some(false)
        { return false; }
        let ctx = t.ctx;
        let Some(code) = choice.code().map(|code| ctx.canonical(code)) else { return true };
        let triggered = matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. });
        if !triggered {
            if let Some(spend) = knowledge::profile(code).counter_engine.and_then(|engine| engine.spend) {
                if spend.description == choice.description {
                    if let knowledge::CounterPayoff::DestroyOnEitherField { domain } = spend.payoff {
                        return ctx.obs.cards.iter().any(|card| card.at.controller == ctx.opp
                            && ctx.reaches(card, code, true, true) && match domain {
                            knowledge::TargetDomain::AnyCard => card.at.location.is_field(),
                            knowledge::TargetDomain::Monster => card.at.location == Location::MonsterZone,
                            knowledge::TargetDomain::SpellTrap => card.at.location == Location::SpellTrapZone,
                        });
                    }
                }
            }
            if let Some(facts) = knowledge::action(code, knowledge::ActionKind::Ignition, choice.description) {
                if let knowledge::Operation::ExtraNormalSummon { setcode, excluded_code } = facts.payoff {
                    // Returning a body only for this restricted extra summon
                    // cannot help when no eligible hand monster exists. The
                    // excluded source cannot make its own bounce productive.
                    return ctx.hand().iter().any(|card| card.code.is_some_and(|candidate| {
                        let data = ctx.data(candidate);
                        ctx.canonical(candidate) != excluded_code && data.is_monster()
                            && !data.is_extra() && data.in_set(setcode)
                            && data.tributes() == 0
                    }));
                }
            }
        }
        true
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

    fn ignition_setup(effect: &EffectDescriptor) -> bool {
        let destructive = Self::REMOVE | cat::RELEASE | cat::FUSION_SUMMON | cat::DAMAGE;
        effect.effect_type & effect_type::IGNITION != 0
            && range::normalize(effect.range) & range::SPELL_TRAP != 0
            && (effect.category == 0
                || effect.category & (Self::ACCESS | cat::SPECIAL_SUMMON | cat::TOKEN) != 0
                    && effect.category & destructive == 0)
    }

    fn ignition_before_summon(&self, t: &Turn, index: usize) -> bool {
        let choice = t.choice(index);
        let Some(code) = choice.code() else { return false };
        let data = t.ctx.data(code);
        data.is_spell() && data.is(types::CONTINUOUS | types::FIELD)
            && choice.at().is_some_and(|at| at.location == Location::SpellTrapZone)
            && self.unknown_activation(t, index).is_some()
            && t.choices().any(|(i, choice)| t.fresh(i) && choice.kind == ChoiceKind::NormalSummon)
            && Self::offered_effects(t, choice).any(Self::ignition_setup)
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
        if alternate::allow_activation(t, index) == Some(false)
            || conversion::allow_activation(t, choice) == Some(false)
            || !Self::known_activation_allowed(t, choice)
        { return false; }
        // Replacing a field can remove a live engine. Evaluating that exchange
        // belongs to search, not to this setup ordering prior.
        if data.is(types::FIELD) && t.ctx.spell_traps(t.ctx.me).iter().any(|card| card.at.sequence == 5) {
            return false;
        }
        let has_normal = t.choices().any(|(i, summon)| t.fresh(i) && summon.kind == ChoiceKind::NormalSummon);
        // A cost-free installation may expose an ignition whose categories
        // are assigned only after choosing a mode. Move an activation already
        // accepted by the fallback earlier; do not infer usefulness from all
        // category-zero field spells (some are locks or forced drawbacks).
        if has_normal && self.unknown_activation(t, index).is_some()
            && Self::offered_effects(t, choice).any(|effect| effect.effect_type & effect_type::ACTIVATE != 0 && !effect.has_cost)
            && t.ctx.db.effects(code).iter().any(Self::ignition_setup)
        {
            return true;
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
        if alternate::allow_activation(t, index) == Some(false)
            || conversion::allow_activation(t, choice) == Some(false)
            || !Self::known_activation_allowed(t, choice)
        {
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
        if Self::shield_useful(t, choice) { return Some(45); }
        // Reactive cards are left to their engine-gated response windows.
        // A legal unknown Trap in Main Phase may consume a defense needlessly.
        let categories = Self::offered_categories(t, choice);
        let backrow_development = at.location == Location::SpellTrapZone
            && categories & (cat::SPECIAL_SUMMON | cat::TOKEN) != 0;
        if (data.is_trap() || data.is(types::QUICKPLAY))
            && categories & Self::ACCESS == 0 && !backrow_development
        {
            return None;
        }
        let score = match at.location {
            Location::Hand if data.is_spell() => 40,
            Location::Hand if data.is_monster() => 35,
            Location::Graveyard | Location::Banished => 35,
            Location::MonsterZone => 30,
            Location::SpellTrapZone if data.is_spell() || categories & Self::DEVELOP != 0 => 25,
            _ => return None,
        };
        Some(score + if categories & Self::ACCESS != 0 { 20 } else { 0 })
    }
}

impl Strategy for Generic {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        if let Some(value) = alternate::value(ctx, code) { return Some(value); }
        if staples::value(code).is_some() {
            return None;
        }
        let data = ctx.data(code);
        data.is_monster().then(|| Self::body_value(ctx, code))
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(index) = alternate::main_phase(t) { return Some(index); }
        // Empty-hand and draw-to-a-limit effects sometimes require backrow
        // to leave the hand first. Only advance a visible, source-backed
        // payoff; ordinary decks keep the shared setting order.
        if let Some(index) = t.choices().find(|(index, choice)| {
            t.fresh(*index) && choice.kind == ChoiceKind::SetSpellTrap
                && choice.code().is_some_and(|code| self.set_spell_trap(t, code) != Some(false))
                && requirements::set_before_payoff(t, choice)
        }).map(|(index, _)| index) {
            return t.pick(index);
        }
        // A summon-triggered engine has to be installed before the event it
        // rewards. The setup card's activation itself often has category zero.
        if let Some(index) = t.choices().find(|(i, _)| self.setup_before_summon(t, *i)).map(|(i, _)| i) {
            return t.pick(index);
        }
        if let Some(index) = t.choices().find(|(i, _)| self.ignition_before_summon(t, *i)).map(|(i, _)| i) {
            return t.pick(index);
        }
        if let Some(index) = t.choices().find(|(index, choice)| {
            self.unknown_activation(t, *index).is_some()
                && requirements::activate_before_normal(t, choice)
        }).map(|(index, _)| index) {
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
        // Equipped monsters and other backrow cards can deploy bodies from
        // that zone. Use the offered action before a summon consumes the host
        // and sends its equipment away. Printed card type is not the action's
        // role. Fusion transactions still need their materials evaluated.
        if let Some(index) = t.choices().filter(|(i, choice)| {
            choice.at().is_some_and(|at| at.location == Location::SpellTrapZone)
                && self.unknown_activation(t, *i).is_some()
                && Self::offered_categories(t, choice) & (cat::SPECIAL_SUMMON | cat::TOKEN) != 0
                && Self::offered_categories(t, choice) & cat::FUSION_SUMMON == 0
        }).map(|(i, _)| i).next() {
            return t.pick(index);
        }
        // Many free hand summons need an empty field. Use that opportunity
        // before spending the normal summon; the engine checks the conditions.
        if t.ctx.monsters(t.ctx.me).is_empty() {
            let choice = t.choices()
                .filter(|(i, c)| t.fresh(*i) && c.kind == ChoiceKind::SpecialSummon)
                .filter(|(_, c)| self.special_summon(t, c) != Some(false))
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
        if let Some(score) = alternate::summon_score(t, choice) { return Some(score); }
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

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        alternate::special_summon(t, choice)
    }

    fn extra_summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let count = requirements::extra_summon_draws(t, choice)?;
        if count == 0 { return None; }
        // A card gained uses the same scale as the normal-summon access
        // reward. This bonus belongs to this summon, not Ragin's keep value.
        let reward = f64::from(count) * 1400.0;
        if tactics::lethal_on_board(t) { return Some(None); }
        let ctx = t.ctx;
        if ctx.main1() && ctx.monsters(ctx.opp).is_empty() {
            // The destination choice does not expose its legal materials.
            // Conservatively price ALL available attackers as consumed,
            // rather than guessing a cheap material pair or zero cost.
            let attack: i64 = ctx.monsters(ctx.me).into_iter().filter(|card| ctx.can_attack(card))
                .map(|card| i64::from(card.attack.max(0))).sum();
            let lost_attack = (attack - i64::from(ctx.data(choice.code()?).attack.max(0))).max(0);
            if reward <= lost_attack as f64 { return None; }
        }
        Some(Some(crate::agent::value(self, &ctx, choice.code(), None) as f64 + reward))
    }

    fn allow_reposition(&self, t: &Turn, card: &CardView) -> bool {
        !card.position.attack || !card.position.face_up || !card.code.is_some_and(|code| {
            knowledge::profile(t.ctx.canonical(code)).dies_in_face_up_defense
        })
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        knowledge::profile(t.ctx.canonical(code)).dies_in_face_up_defense.then_some(Position::FACE_UP_ATTACK)
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        alternate::allow_staple(t, code)
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        alternate::set_spell_trap(t, code)
    }

    fn allow_repeated_chain(&self, t: &Turn, index: usize) -> bool {
        alternate::allow_repeated_chain(t, index)
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        // The engine's attack choice says whether this specific attacker may
        // bypass opposing monsters. Preserve an empty target intent so the
        // ensuing "attack directly?" prompt follows that legal plan.
        let index = tactics::direct_attack(t)?;
        t.pick_targeting(index, vec![])
    }

    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        combat::attack_trick(ctx, attacker)
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        combat::attack_outcome(ctx, attacker, target)
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        if let Some(response) = alternate::chain(t, index) { return Some(response); }
        if let Some(response) = combat::chain(t, index) { return Some(response); }
        let choice = t.choice(index);
        if alternate::allow_activation(t, index) == Some(false)
            || conversion::allow_activation(t, choice) == Some(false)
            || !Self::known_activation_allowed(t, choice)
        {
            return Some(Response::no());
        }
        if transactions::allow_activation(t, choice) == Some(true) {
            // A precisely identified profitable transaction can also share
            // our trigger chain; unrelated optional effects keep their guard.
            return Some(Response::new(110.0));
        }
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
        if Self::shield_useful(t, choice) { return Some(Response::new(45.0)); }
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
        if let Some(score) = alternate::select(t, member) { return Some(score); }
        if let Some(score) = materials::material_score(t, member) { return Some(score); }
        if let Some(score) = conversion::material_score(t, member) { return Some(score); }
        if let Some(score) = transactions::select(t, member) { return Some(score); }
        let code = member.code?;
        let ctx = t.ctx;
        let hint = t.decision.hint;
        if matches!(hint, Hint::FaceUp | Hint::Target) {
            if let Some(support) = t.memory.last_activated.and_then(|source| knowledge::profile(ctx.canonical(source)).counter_support) {
                if support.recipients == knowledge::CounterRecipients::OneFaceUpMonsterOnEitherField {
                    let engine = knowledge::profile(ctx.canonical(code)).counter_engine;
                    return Some(if member.at.controller != ctx.me { -10_000.0 }
                        else if engine.is_some_and(|engine| engine.counter == support.counter) { 10_000.0 }
                        else { 0.0 });
                }
            }
        }
        if member.at.controller != ctx.me {
            return None;
        }
        let worth = Self::body_value(&ctx, code) as f64;
        if matches!(hint, Hint::ToGraveyard | Hint::Discard | Hint::FusionMaterial | Hint::SynchroMaterial | Hint::Tribute | Hint::Release) {
            let arrival = requirements::arrival_value(t, member);
            let grave = arrival.unwrap_or_else(|| Self::grave_utility(&ctx, code));
            if arrival.is_some() || grave > 0.0 || hint == Hint::ToGraveyard && member.at.location == Location::Deck {
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

    fn continue_optional_selection(&self, t: &Turn) -> bool {
        materials::continue_optional_selection(t)
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        alternate::yes_no(t)
            .or_else(|| transactions::yes_no(t))
            .or_else(|| tactics::attack_directly(t))
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
    fn source_backed_damage_shield_requires_a_body_and_no_live_trap_negator() {
        let shield = 53239672;
        let body = 900005001;
        let database = profiled(vec![
            CardData { code: shield, kind: types::TRAP | types::CONTINUOUS, ..CardData::default() },
            CardData { code: body, kind: types::MONSTER, ..CardData::default() },
            CardData { code: 77585513, kind: types::MONSTER | types::EFFECT, ..CardData::default() },
        ], EffectProfiles::default());
        for (has_body, negated, expected) in [(true, false, 0), (false, false, 1), (true, true, 1)] {
            let mut obs = observation();
            obs.cards.push(on_field(shield, 0, Location::SpellTrapZone, Position::FACE_DOWN_DEFENSE));
            if has_body { obs.cards.push(on_field(body, 0, Location::MonsterZone, Position::FACE_UP_ATTACK)); }
            if negated { obs.cards.push(on_field(77585513, 1, Location::MonsterZone, Position::FACE_UP_ATTACK)); }
            for kind in [DecisionKind::Idle, DecisionKind::Chain { triggers: false, forced: false }] {
                let end = if kind == DecisionKind::Idle { ChoiceKind::EndTurn } else { ChoiceKind::Pass };
                let prompt = decision(kind, vec![choice(ChoiceKind::Activate, Some(shield), Location::SpellTrapZone), choice(end, None, Location::Other)]);
                let mut pilot = Agent::seeded(Generic, database.clone(), 1);
                assert_eq!(pilot.choose(&obs, &prompt), expected, "{kind:?}, body={has_body}, negated={negated}");
            }
        }
    }

    #[test]
    fn counter_summon_resources_and_self_destruction_change_position_choices() {
        let body = 16197610;
        let database = profiled(vec![CardData { code: body, kind: types::MONSTER | types::EFFECT,
            attack: 800, defense: 0, level: 4, ..CardData::default() }], EffectProfiles::default());
        let mut obs = observation();
        obs.cards.push(on_field(900005011, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::SetMonster, Some(body), Location::Hand),
            choice(ChoiceKind::NormalSummon, Some(body), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), 1);
        let mut prompt = decision(DecisionKind::Position, vec![
            choice(ChoiceKind::Position(Position::FACE_UP_DEFENSE), None, Location::Other),
            choice(ChoiceKind::Position(Position::FACE_UP_ATTACK), None, Location::Other),
        ]);
        prompt.subject = Some(body);
        assert_eq!(Agent::seeded(Generic, database, 1).choose(&obs, &prompt), 1);
    }

    #[test]
    fn restricted_extra_summon_does_not_bounce_for_an_ineligible_hand() {
        let source = 9126351;
        let candidate = 900005021;
        for (same_set, level, allowed) in [(false, 4, false), (true, 7, false), (true, 4, true)] {
            let database = profiled(vec![
                CardData { code: source, kind: types::MONSTER | types::EFFECT, level: 2, ..CardData::default() },
                CardData { code: candidate, kind: types::MONSTER, level, setcodes: if same_set { vec![knowledge::FROG_SET] } else { vec![] }, ..CardData::default() },
            ], EffectProfiles::default());
            let mut obs = observation();
            obs.cards.push(on_field(source, 0, Location::MonsterZone, Position::FACE_UP_ATTACK));
            obs.cards.push(on_field(candidate, 0, Location::Hand, Position::FACE_UP_ATTACK));
            let mut action = choice(ChoiceKind::Activate, Some(source), Location::MonsterZone);
            action.description = knowledge::description(source, 1);
            let prompt = decision(DecisionKind::Idle, vec![action.clone(), choice(ChoiceKind::EndTurn, None, Location::Other)]);
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), usize::from(!allowed));
            // The summon trigger deliberately shares the same description;
            // its graveyard setup is a different effect and remains allowed.
            let trigger = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![action, choice(ChoiceKind::Pass, None, Location::Other)]);
            assert_eq!(Agent::seeded(Generic, database, 1).choose(&obs, &trigger), 0);
        }
    }

    #[test]
    fn counter_removal_does_not_spend_resources_on_our_only_target() {
        let source = 43318266;
        let database = profiled(vec![CardData { code: source, kind: types::MONSTER | types::EFFECT, ..CardData::default() }], EffectProfiles::default());
        for (enemy, allowed) in [(None, false), (Some(900005031), true), (Some(27527047), false)] {
            let mut obs = observation();
            obs.cards.push(on_field(source, 0, Location::MonsterZone, Position::FACE_UP_ATTACK));
            if let Some(enemy) = enemy { obs.cards.push(on_field(enemy, 1, Location::MonsterZone, Position::FACE_UP_ATTACK)); }
            let mut action = choice(ChoiceKind::Activate, Some(source), Location::MonsterZone);
            action.description = knowledge::description(source, 1);
            let prompt = decision(DecisionKind::Idle, vec![action, choice(ChoiceKind::EndTurn, None, Location::Other)]);
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), usize::from(!allowed));
        }
    }

    #[test]
    fn setup_ordering_respects_alternate_win_backrow_reservation() {
        let setup = 900005041;
        let body = 900005042;
        let database = profiled(vec![
            CardData { code: setup, kind: types::SPELL | types::CONTINUOUS, ..CardData::default() },
            CardData { code: body, kind: types::MONSTER, attack: 1900, level: 4, ..CardData::default() },
        ], [(setup, vec![EffectDescriptor { category: cat::SEARCH, effect_type: effect_type::TRIGGER,
            range: range::SPELL_TRAP, event: event::SUMMON_SUCCESS, ..EffectDescriptor::default() }])].into());
        let mut obs = observation();
        obs.cards.push(on_field(94212438, 0, Location::Hand, Position::FACE_UP_ATTACK));
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::Activate, Some(setup), Location::Hand),
            choice(ChoiceKind::NormalSummon, Some(body), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), 1);
        let chain = decision(DecisionKind::Chain { forced: false, triggers: false }, vec![
            choice(ChoiceKind::Activate, Some(setup), Location::SpellTrapZone),
            choice(ChoiceKind::Pass, None, Location::Other),
        ]);
        assert_eq!(Agent::seeded(Generic, database, 1).choose(&obs, &chain), 1);
    }

    #[test]
    fn discard_trigger_credit_depends_on_the_actual_origin_and_reason() {
        let discarded = 82888408;
        let inert = 900006001;
        let database = profiled(vec![
            CardData { code: discarded, kind: types::MONSTER | types::EFFECT | types::TUNER,
                attack: 1000, defense: 400, level: 2, ..CardData::default() },
            CardData { code: inert, kind: types::MONSTER, attack: 400, level: 2, ..CardData::default() },
        ], [(discarded, vec![EffectDescriptor {
            category: cat::SPECIAL_SUMMON, effect_type: effect_type::TRIGGER,
            event: event::TO_GRAVE, description: knowledge::description(discarded, 0),
            ..EffectDescriptor::default()
        }])].into());
        for (hint, location, expected) in [
            (Hint::Discard, Location::Hand, 0),
            (Hint::ToGraveyard, Location::Hand, 1),
            (Hint::ToGraveyard, Location::Deck, 1),
            (Hint::SynchroMaterial, Location::MonsterZone, 1),
            (Hint::FusionMaterial, Location::Hand, 1),
        ] {
            let mut obs = observation();
            let candidates = [discarded, inert].into_iter().enumerate().map(|(sequence, code)| {
                let mut view = on_field(code, 0, location, Position::FACE_UP_ATTACK);
                view.at.sequence = sequence as u32;
                view.attack = database.card(code).unwrap().attack;
                view.defense = database.card(code).unwrap().defense;
                obs.cards.push(view);
                let mut candidate = choice(ChoiceKind::Cards, Some(code), location);
                candidate.card.as_mut().unwrap().at.sequence = sequence as u32;
                candidate.members.push(candidate.card.take().unwrap());
                candidate
            }).collect();
            let mut prompt = decision(DecisionKind::SelectCards, candidates);
            prompt.hint = hint;
            prompt.minimum = 1;
            prompt.maximum = 1;
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), expected, "{hint:?} from {location:?}");
        }
    }

    #[test]
    fn profitable_hand_extender_discards_before_committing_the_normal_summon() {
        let extender = 29905795;
        let fuel = 82888408;
        let ordinary = 900007021;
        let inert_fuel = 900007022;
        let database = profiled(vec![
            CardData { code: extender, kind: types::MONSTER | types::EFFECT | types::TUNER,
                attack: 200, level: 1, setcodes: vec![0x35], ..CardData::default() },
            CardData { code: fuel, kind: types::MONSTER | types::EFFECT | types::TUNER,
                attack: 1000, level: 2, setcodes: vec![0x35], ..CardData::default() },
            CardData { code: ordinary, kind: types::MONSTER, attack: 1900, level: 4, setcodes: vec![0x35], ..CardData::default() },
            CardData { code: inert_fuel, kind: types::MONSTER, attack: 1000, level: 2, setcodes: vec![0x35], ..CardData::default() },
        ], EffectProfiles::default());
        for profitable in [false, true] {
            let mut obs = observation();
            for (sequence, code) in [extender, if profitable { fuel } else { inert_fuel }, ordinary].into_iter().enumerate() {
                let mut card = on_field(code, 0, Location::Hand, Position::FACE_UP_ATTACK);
                card.at.sequence = sequence as u32;
                obs.cards.push(card);
            }
            let mut activate = choice(ChoiceKind::Activate, Some(extender), Location::Hand);
            activate.description = knowledge::description(extender, 0);
            let mut summon = choice(ChoiceKind::NormalSummon, Some(ordinary), Location::Hand);
            summon.card.as_mut().unwrap().at.sequence = 2;
            let prompt = decision(DecisionKind::Idle, vec![summon.clone(), activate, choice(ChoiceKind::EndTurn, None, Location::Other)]);
            let mut pilot = Agent::seeded(Generic, database.clone(), 1);
            assert_eq!(pilot.choose(&obs, &prompt), usize::from(profitable));
            if !profitable { continue; }
            let selections = [fuel, ordinary].into_iter().enumerate().map(|(index, code)| {
                let mut candidate = choice(ChoiceKind::Cards, Some(code), Location::Hand);
                candidate.card.as_mut().unwrap().at.sequence = index as u32 + 1;
                candidate.members.push(candidate.card.take().unwrap());
                candidate
            }).collect();
            let mut discard = decision(DecisionKind::SelectCards, selections);
            discard.hint = Hint::Discard;
            discard.minimum = 1;
            discard.maximum = 1;
            assert_eq!(pilot.choose(&obs, &discard), 0);
            // Simulate completion of the two legal summons, then verify that
            // the retained unfamiliar monster still receives the Normal Summon.
            obs.cards[0].at.location = Location::MonsterZone;
            obs.cards[1].at.location = Location::MonsterZone;
            let followup = decision(DecisionKind::Idle, vec![summon, choice(ChoiceKind::EndTurn, None, Location::Other)]);
            assert_eq!(pilot.choose(&obs, &followup), 0);
        }
    }

    #[test]
    fn extra_summon_ranking_values_only_the_available_ragin_refill() {
        let ragin = 47395382;
        let unfamiliar = 900007051;
        let database = profiled(vec![
            CardData { code: ragin, kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 2300, defense: 1800, level: 5, ..CardData::default() },
            CardData { code: unfamiliar, kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 2900, level: 8, ..CardData::default() },
        ], EffectProfiles::default());
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::SpecialSummon, Some(ragin), Location::Extra),
            choice(ChoiceKind::SpecialSummon, Some(unfamiliar), Location::Extra),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        for (hand, deck, drained, expected) in [
            (0, 20, false, 0), (1, 20, false, 0), (2, 20, false, 1),
            (3, 20, false, 1), (0, 1, false, 1), (1, 0, false, 1),
            (0, 20, true, 1),
        ] {
            let mut obs = observation();
            obs.cards.push(on_field(900007052, 1, Location::MonsterZone, Position::FACE_UP_ATTACK));
            obs.pile_sizes = vec![(0, Location::Hand, hand), (0, Location::Deck, deck)];
            if drained {
                obs.cards.push(on_field(crate::knowledge::SKILL_DRAIN, 1, Location::SpellTrapZone, Position::FACE_UP_ATTACK));
            }
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), expected,
                "hand={hand}, deck={deck}, drained={drained}");
            let mut memory = crate::agent::Memory::default();
            let turn = Turn { ctx: Ctx::new(&obs, database.as_ref()), decision: &prompt, memory: &mut memory };
            assert_eq!(Generic.extra_summon_score(&turn, &prompt.choices[1]), None,
                "an unfamiliar summon retains the ordinary ranking");
        }
    }

    #[test]
    fn ragin_refill_must_cover_all_available_attack_lost_and_never_delays_lethal() {
        let ragin = 47395382;
        let database = profiled(vec![CardData { code: ragin,
            kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
            attack: 2300, defense: 1800, level: 5, ..CardData::default() }], EffectProfiles::default());
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::SpecialSummon, Some(ragin), Location::Extra),
            choice(ChoiceKind::EnterBattle, None, Location::Other),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        for (hand, attack, life, expected) in [
            (0, 5000, 8000, 0), (0, 6000, 8000, 1),
            (1, 3000, 8000, 0), (1, 4000, 8000, 1),
            (1, 3700, 8000, 1), (2, 3000, 8000, 1),
            (0, 5000, 5000, 1),
        ] {
            let mut obs = observation();
            obs.pile_sizes = vec![(0, Location::Hand, hand), (0, Location::Deck, 20)];
            obs.life_points[1] = life;
            for sequence in 0..2 {
                let mut body = on_field(900007053, 0, Location::MonsterZone, Position::FACE_UP_ATTACK);
                body.at.sequence = sequence;
                body.attack = attack / 2;
                obs.cards.push(body);
            }
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), expected,
                "hand={hand}, attack={attack}, opponent_lp={life}");
        }
    }

    #[test]
    fn early_backrow_setting_requires_a_visible_empty_hand_payoff() {
        let spell = 900006011;
        let target = 900006012;
        let engine = 66957584;
        let database = profiled(vec![
            CardData { code: spell, kind: types::SPELL, ..CardData::default() },
            CardData { code: target, kind: types::MONSTER, level: 4, setcodes: vec![0xb], ..CardData::default() },
            CardData { code: engine, kind: types::SPELL | types::CONTINUOUS, ..CardData::default() },
        ], EffectProfiles::default());
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::SetSpellTrap, Some(spell), Location::Hand),
            choice(ChoiceKind::Activate, Some(spell), Location::Hand),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        for active in [false, true] {
            let mut obs = observation();
            obs.cards.push(on_field(spell, 0, Location::Hand, Position::FACE_UP_ATTACK));
            obs.cards.push(on_field(target, 0, Location::Graveyard, Position::FACE_UP_ATTACK));
            if active { obs.cards.push(on_field(engine, 0, Location::SpellTrapZone, Position::FACE_UP_ATTACK)); }
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), usize::from(!active));
        }
    }

    #[test]
    fn opponent_draw_spell_does_not_treat_our_only_board_as_a_draw_engine() {
        let source = 35480699;
        let body = 900006021;
        let database = profiled(vec![
            CardData { code: source, kind: types::SPELL | types::QUICKPLAY, ..CardData::default() },
            CardData { code: body, kind: types::MONSTER, attack: 1800, level: 4, ..CardData::default() },
        ], [(source, vec![EffectDescriptor { category: cat::DRAW,
            effect_type: effect_type::ACTIVATE, ..EffectDescriptor::default() }])].into());
        for enemy in [false, true] {
            let mut obs = observation();
            obs.cards.push(on_field(body, 0, Location::MonsterZone, Position::FACE_UP_ATTACK));
            if enemy { obs.cards.push(on_field(body, 1, Location::MonsterZone, Position::FACE_UP_ATTACK)); }
            for kind in [DecisionKind::Idle, DecisionKind::Chain { triggers: false, forced: false }] {
                let end = if kind == DecisionKind::Idle { ChoiceKind::EndTurn } else { ChoiceKind::Pass };
                let prompt = decision(kind, vec![choice(ChoiceKind::Activate, Some(source), Location::Hand), choice(end, None, Location::Other)]);
                assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), usize::from(!enemy));
            }
        }
    }

    #[test]
    fn sole_attacker_is_preserved_when_its_costly_removal_replaces_a_safe_battle() {
        let source = 10026986;
        let enemy = 900006031;
        let database = profiled(vec![
            CardData { code: source, kind: types::MONSTER | types::EFFECT, attack: 2700, defense: 1100,
                level: 8, race: crate::cards::races::REPTILE, setcodes: vec![0x3e], ..CardData::default() },
            CardData { code: enemy, kind: types::MONSTER, attack: 1200, defense: 500, level: 4, ..CardData::default() },
        ], EffectProfiles::default());
        let mut obs = observation();
        let mut king = on_field(source, 0, Location::MonsterZone, Position::FACE_UP_ATTACK);
        king.attack = 2700;
        king.defense = 1100;
        let mut target = on_field(enemy, 1, Location::MonsterZone, Position::FACE_UP_ATTACK);
        target.attack = 1200;
        target.defense = 500;
        obs.cards.extend([king, target]);
        let mut action = choice(ChoiceKind::Activate, Some(source), Location::MonsterZone);
        action.description = knowledge::description(source, 1);
        let prompt = decision(DecisionKind::Idle, vec![action,
            choice(ChoiceKind::EnterBattle, None, Location::Other), choice(ChoiceKind::EndTurn, None, Location::Other)]);
        assert_eq!(Agent::seeded(Generic, database, 1).choose(&obs, &prompt), 1);
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
    fn summon_reward_requires_the_trigger_to_exist_at_the_summoned_destination() {
        let trigger_body = 900007001;
        let ordinary_body = 900007002;
        for (source_range, source_type, source_event, rewarded) in [
            (0, effect_type::TRIGGER, event::SUMMON_SUCCESS, true),
            (range::MONSTER, effect_type::TRIGGER, event::SUMMON_SUCCESS, true),
            (range::MONSTER_MAIN, effect_type::TRIGGER, event::SUMMON_SUCCESS, true),
            // FIELD+TRIGGER from HAND mirrors an extender that must remain
            // there while another monster is Normal Summoned.
            (range::HAND, effect_type::TRIGGER | 0x2, event::SUMMON_SUCCESS, false),
            (range::GRAVEYARD, effect_type::TRIGGER, event::SUMMON_SUCCESS, false),
            (range::BANISHED, effect_type::TRIGGER, event::SUMMON_SUCCESS, false),
            (range::SPELL_TRAP, effect_type::TRIGGER, event::SUMMON_SUCCESS, false),
            (range::MONSTER_EXTRA, effect_type::TRIGGER, event::SUMMON_SUCCESS, false),
            (range::MONSTER, effect_type::IGNITION, event::SUMMON_SUCCESS, false),
            (range::MONSTER, effect_type::TRIGGER, event::SPECIAL_SUMMON_SUCCESS, false),
        ] {
            let database = profiled(vec![
                CardData { code: trigger_body, kind: types::MONSTER | types::EFFECT,
                    attack: 1500, defense: 1000, level: 4, ..CardData::default() },
                CardData { code: ordinary_body, kind: types::MONSTER,
                    attack: 2000, defense: 1000, level: 4, ..CardData::default() },
            ], [(trigger_body, vec![EffectDescriptor {
                category: cat::SPECIAL_SUMMON, effect_type: source_type, event: source_event,
                range: source_range, ..EffectDescriptor::default()
            }])].into());
            let mut obs = observation();
            obs.cards.push(on_field(trigger_body, 0, Location::Hand, Position::FACE_UP_ATTACK));
            let mut other = on_field(ordinary_body, 0, Location::Hand, Position::FACE_UP_ATTACK);
            other.at.sequence = 1;
            obs.cards.push(other);
            let mut other_summon = choice(ChoiceKind::NormalSummon, Some(ordinary_body), Location::Hand);
            other_summon.card.as_mut().unwrap().at.sequence = 1;
            let prompt = decision(DecisionKind::Idle, vec![
                choice(ChoiceKind::NormalSummon, Some(trigger_body), Location::Hand),
                other_summon, choice(ChoiceKind::EndTurn, None, Location::Other),
            ]);
            assert_eq!(Agent::seeded(Generic, database, 1).choose(&obs, &prompt), usize::from(!rewarded),
                "range={source_range:#x}, type={source_type:#x}, event={source_event}");
        }
    }

    #[test]
    fn known_summon_wipe_joins_our_chain_only_for_a_profitable_public_exchange() {
        let source = 73580471;
        let enemy = 900007011;
        let database = profiled(vec![
            CardData { code: source, kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 2400, defense: 1800, level: 7, ..CardData::default() },
            CardData { code: enemy, kind: types::MONSTER, attack: 2800, defense: 2000, level: 4, ..CardData::default() },
        ], EffectProfiles::default());
        for profitable in [false, true] {
            let mut obs = observation();
            let mut dragon = on_field(source, 0, Location::MonsterZone, Position::FACE_UP_ATTACK);
            dragon.attack = 2400;
            dragon.defense = 1800;
            obs.cards.push(dragon);
            for sequence in 0..if profitable { 2 } else { 1 } {
                let mut target = on_field(enemy, 1, Location::MonsterZone,
                    if profitable { Position::FACE_UP_ATTACK } else { Position::FACE_DOWN_DEFENSE });
                target.at.sequence = sequence;
                target.attack = 2800;
                target.defense = 2000;
                if !profitable { target.code = None; }
                obs.cards.push(target);
            }
            obs.chain.push(ChainLink { code: 900007012, controller: 0,
                source: CardRef { controller: 0, location: Location::SpellTrapZone, sequence: 4 }, targets: vec![] });
            let mut effect = choice(ChoiceKind::Activate, Some(source), Location::MonsterZone);
            effect.description = knowledge::description(source, 0);
            let prompt = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
                effect, choice(ChoiceKind::Pass, None, Location::Other),
            ]);
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &prompt), usize::from(!profitable));
            // Native single-trigger questions use system description 221,
            // rather than the card's script description used by Chain.
            obs.chain.clear();
            let mut yes = choice(ChoiceKind::Yes, Some(source), Location::MonsterZone);
            yes.description = 221;
            let mut yes_no = decision(DecisionKind::YesNo, vec![yes, choice(ChoiceKind::No, None, Location::Other)]);
            yes_no.subject = Some(source);
            assert_eq!(Agent::seeded(Generic, database.clone(), 1).choose(&obs, &yes_no), usize::from(!profitable));
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
    fn unknown_mode_setup_and_its_ignition_both_precede_the_normal_summon() {
        let setup = 900005001;
        let monster = 900005002;
        for (category, activation_cost, occupied, has_ignition, promote) in [
            (0, false, false, true, true),
            (cat::SPECIAL_SUMMON, false, false, true, true),
            (cat::SPECIAL_SUMMON | cat::DESTROY, false, false, true, false),
            (cat::SPECIAL_SUMMON | cat::FUSION_SUMMON, false, false, true, false),
            (0, true, false, true, false),
            (0, false, true, true, false),
            (0, false, false, false, false),
        ] {
            let mut effects = vec![EffectDescriptor {
                effect_type: effect_type::ACTIVATE, has_cost: activation_cost, ..EffectDescriptor::default()
            }];
            if has_ignition {
                effects.push(EffectDescriptor {
                    category, effect_type: effect_type::IGNITION, range: range::FIELD,
                    description: 91, has_cost: true, ..EffectDescriptor::default()
                });
            }
            let database = profiled(vec![
                CardData { code: setup, kind: types::SPELL | types::FIELD, ..CardData::default() },
                CardData { code: monster, kind: types::MONSTER, attack: 1800, level: 4, ..CardData::default() },
            ], [(setup, effects)].into());
            let mut pilot = Agent::seeded(Generic, database, 1);
            let mut obs = observation();
            if occupied {
                let mut field = on_field(900005003, 0, Location::SpellTrapZone, Position::FACE_UP_ATTACK);
                field.at.sequence = 5;
                obs.cards.push(field);
            }
            let initial = decision(DecisionKind::Idle, vec![
                choice(ChoiceKind::NormalSummon, Some(monster), Location::Hand),
                choice(ChoiceKind::Activate, Some(setup), Location::Hand),
                choice(ChoiceKind::EndTurn, None, Location::Other),
            ]);
            assert_eq!(pilot.choose(&obs, &initial), usize::from(promote),
                "category={category}, activation_cost={activation_cost}, occupied={occupied}, ignition={has_ignition}");
            if promote {
                let mut field = on_field(setup, 0, Location::SpellTrapZone, Position::FACE_UP_ATTACK);
                field.at.sequence = 5;
                obs.cards.push(field);
                let mut activation = choice(ChoiceKind::Activate, Some(setup), Location::SpellTrapZone);
                activation.card.as_mut().unwrap().at.sequence = 5;
                activation.description = 91;
                let followup = decision(DecisionKind::Idle, vec![
                    choice(ChoiceKind::NormalSummon, Some(monster), Location::Hand),
                    activation,
                    choice(ChoiceKind::EndTurn, None, Location::Other),
                ]);
                assert_eq!(pilot.choose(&obs, &followup), 1, "the new engine must act before spending the summon");
            }
        }
    }

    #[test]
    fn productive_equipped_monster_acts_before_a_summon_can_spend_its_host() {
        let equipped = 900004001;
        let host = 900004002;
        let summon = 900004003;
        let database = profiled(vec![
            CardData { code: equipped, kind: types::MONSTER | types::EFFECT | types::TUNER, attack: 500, level: 2, ..CardData::default() },
            CardData { code: host, kind: types::MONSTER | types::EFFECT, attack: 1900, level: 4, ..CardData::default() },
            CardData { code: summon, kind: types::MONSTER | types::EFFECT, attack: 2600, level: 8, ..CardData::default() },
        ], [(equipped, vec![EffectDescriptor {
            category: cat::SPECIAL_SUMMON, effect_type: effect_type::IGNITION,
            range: range::SPELL_TRAP, ..EffectDescriptor::default()
        }])].into());
        let mut obs = observation();
        obs.cards.push(on_field(host, 0, Location::MonsterZone, Position::FACE_UP_ATTACK));
        obs.cards.push(on_field(equipped, 0, Location::SpellTrapZone, Position::FACE_UP_ATTACK));
        let mut pilot = Agent::seeded(Generic, database, 1);
        let prompt = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::SpecialSummon, Some(summon), Location::Hand),
            choice(ChoiceKind::Activate, Some(equipped), Location::SpellTrapZone),
            choice(ChoiceKind::EndTurn, None, Location::Other),
        ]);
        assert_eq!(pilot.choose(&obs, &prompt), 1);
        let alone = decision(DecisionKind::Idle, vec![
            choice(ChoiceKind::EndTurn, None, Location::Other),
            choice(ChoiceKind::Activate, Some(equipped), Location::SpellTrapZone),
        ]);
        assert_eq!(pilot.choose(&obs, &alone), 1);
    }

    #[test]
    fn unseen_direct_attacker_uses_engine_permission_and_confirms_direct_attack() {
        let attacker = 900004011;
        let database = Arc::new(MemoryCards([(attacker, CardData {
            code: attacker, kind: types::MONSTER, attack: 1400, level: 4, ..CardData::default()
        })].into()));
        for can_direct in [false, true] {
            let mut pilot = Agent::seeded(Generic, database.clone(), 1);
            let mut obs = observation();
            obs.phase = Some(Phase::BattleStart);
            let mut own = on_field(attacker, 0, Location::MonsterZone, Position::FACE_UP_ATTACK);
            own.attack = 1400;
            let mut enemy = on_field(900004012, 1, Location::MonsterZone, Position::FACE_UP_ATTACK);
            enemy.attack = 2600;
            obs.cards.extend([own, enemy]);
            let mut attack = choice(ChoiceKind::Attack, Some(attacker), Location::MonsterZone);
            attack.card.as_mut().unwrap().value = i64::from(can_direct);
            let prompt = decision(DecisionKind::Battle, vec![
                attack, choice(ChoiceKind::EnterMain2, None, Location::Other),
            ]);
            assert_eq!(pilot.choose(&obs, &prompt), usize::from(!can_direct));
            if can_direct {
                let mut confirmation = decision(DecisionKind::YesNo, vec![
                    choice(ChoiceKind::No, None, Location::Other),
                    choice(ChoiceKind::Yes, None, Location::Other),
                ]);
                for answer in &mut confirmation.choices { answer.description = tactics::ATTACK_DIRECTLY; }
                assert_eq!(pilot.choose(&obs, &confirmation), 1);
            }
        }
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
