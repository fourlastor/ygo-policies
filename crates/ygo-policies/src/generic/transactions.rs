//! Narrow resource guards for known card transactions, using only public state.
//! These are card rules and reusable exchange tests, never deck dispatch.
//!
//! Sources: `vendor/CardScripts/official/c35480699.lua` (Eclipse),
//! `c10026986.lua` (King), `c73580471.lua` (Black Rose), `c57774843.lua`
//! (Judgment Dragon), `c29071332.lua` (Armory Arm), `c39402797.lua`
//! (Trident Dragion), and the
//! mandatory-removal facts in `knowledge`.
//! An ordinary NormalSummon choice does not expose its actual tribute cost.
//! Consequently this module never vetoes a summon from printed Level alone.

use super::{knowledge, Generic};
use crate::agent::{self, Outcome, Turn};
use crate::cards::{attributes, races, types};
use crate::model::{CardRef, CardView, Choice, ChoiceKind, DecisionKind, Hint, Location, Member};
use crate::tactics;

const BOOK_OF_ECLIPSE: u32 = 35480699;
const WORM_KING: u32 = 10026986;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const JUDGMENT_DRAGON: u32 = 57774843;
const ARMORY_ARM: u32 = 29071332;
const TRIDENT_DRAGION: u32 = 39402797;
const WORM_SET: u16 = 0x3e;

/// Partial transaction facts only. Every identity occurs in the active pool.
pub const SUPPORTED_CODES: &[u32] = &[
    9748752,
    WORM_KING,
    BOOK_OF_ECLIPSE,
    73125233,
    BLACK_ROSE_DRAGON,
    JUDGMENT_DRAGON,
    ARMORY_ARM,
    TRIDENT_DRAGION,
];

fn keep_value(t: &Turn, card: &CardView) -> i32 {
    agent::value(&Generic, &t.ctx, card.code, Some(card))
}

fn monster_source(t: &Turn, choice: &Choice, source: u32) -> bool {
    choice.code().map(|code| t.ctx.canonical(code)) == Some(source)
        && choice
            .at()
            .is_some_and(|at| at.controller == t.ctx.me && at.location == Location::MonsterZone)
        && t.view(choice)
            .is_some_and(|card| card.position.face_up && t.ctx.is(card, source))
}

/// A conservative exchange estimate, not a simulator of destruction triggers.
/// Unknown opposing facedowns retain the public uncertainty values supplied
/// by sweep_worth. Own cards are charged their full keep value: a generic GY
/// payoff is not guaranteed when destruction/replacement conditions are opaque.
fn global_destruction_profitable(
    t: &Turn,
    source: u32,
    excluded: Option<CardRef>,
    lp_cost: i32,
) -> bool {
    let mut removed = 0i64;
    let mut lost = i64::from(lp_cost);
    for card in t
        .ctx
        .obs
        .cards
        .iter()
        .filter(|card| card.at.location.is_field() && Some(card.at) != excluded)
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

/// Optional Synchro-arrival effects use native YesNo descriptions 0/221.
/// Black Rose's wipe and Trident's extra attacks each have slot 0. Other
/// questions and Black Rose's Plant-banish ignition retain their old behavior.
pub(super) fn yes_no(t: &Turn) -> Option<bool> {
    if t.decision.kind != DecisionKind::YesNo {
        return None;
    }
    let source = t.ctx.canonical(t.decision.subject?);
    if !matches!(source, BLACK_ROSE_DRAGON | TRIDENT_DRAGION) {
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
    if !monster_source(t, choice, source)
        || !(matches!(choice.description, 0 | 221)
            || choice.description == knowledge::description(source, 0))
    {
        return None;
    }
    Some(if source == TRIDENT_DRAGION {
        trident(t, t.view(choice)?)
    } else {
        global_destruction_profitable(t, source, None, 0)
    })
}

fn judgment(t: &Turn, choice: &Choice) -> bool {
    t.ctx.my_lp() > 1000
        && !t.ctx.effects_drained()
        && !t.ctx.wasted(JUDGMENT_DRAGON)
        && global_destruction_profitable(t, JUDGMENT_DRAGON, choice.at(), 1000)
}

/// Install this in the early generic main-phase hook, before committing new
/// bodies/backrow to an already-profitable all-other-cards destruction effect.
pub(super) fn activate_before_development(t: &Turn, choice: &Choice) -> bool {
    t.decision.kind == DecisionKind::Idle
        && t.ctx.main1()
        && t.ctx.my_turn()
        && t.ctx.obs.chain.is_empty()
        && choice.kind == ChoiceKind::Activate
        && monster_source(t, choice, JUDGMENT_DRAGON)
        && choice.description == knowledge::description(JUDGMENT_DRAGON, 1)
        && judgment(t, choice)
}

/// Only a visible battle with no paid trick, known replacement body, reflected
/// damage, or negation gets credited. Hidden targets are not assumed cleared.
fn safe_battle_value(t: &Turn, attacker: &CardView, target: &CardView) -> Option<i32> {
    if !target.position.face_up
        || !target.known()
        || t.ctx.attack_negatable(target)
        || !matches!(
            tactics::default_outcome(&t.ctx, attacker, target, 0),
            Outcome::Win { trick: false }
        )
    {
        return None;
    }
    let met = t.ctx.attack_meets(Some(attacker), target);
    if met.facts.payoff > 0
        || met.facts.collateral > 0
        || met.facts.burn != crate::knowledge::Burn::None
    {
        return None;
    }
    let damage = safe_battle_damage(t, attacker, target);
    Some(t.ctx.threat(target).max(0) + damage.min(t.ctx.opp_lp().max(0)))
}

fn safe_battle_damage(t: &Turn, attacker: &CardView, target: &CardView) -> i32 {
    let met = t.ctx.attack_meets(Some(attacker), target);
    let shielded = t.ctx.obs.cards.iter().any(|source| {
        if source.at.location != Location::SpellTrapZone || !source.position.face_up {
            return false;
        }
        let data = t.ctx.view_data(source);
        if data.is(types::FIELD) && source.at.sequence != 5 {
            return false;
        }
        if data.is_trap()
            && t.ctx.obs.cards.iter().any(|negator| {
                let data = t.ctx.view_data(negator);
                let active_zone = data.is_monster() && negator.at.location == Location::MonsterZone
                    || (data.is_spell() || data.is_trap())
                        && negator.at.location == Location::SpellTrapZone;
                active_zone
                    && negator.position.face_up
                    && (negator.at.location != Location::MonsterZone || !t.ctx.effects_drained())
                    && negator.code.is_some_and(|code| {
                        knowledge::profile(t.ctx.canonical(code)).negates_field_traps
                    })
            })
        {
            return false;
        }
        match source
            .code
            .and_then(|code| knowledge::profile(t.ctx.canonical(code)).damage_shield)
        {
            Some(knowledge::DamageShield::ControllerWhileControlsMonster) => {
                source.at.controller == target.at.controller
            }
            Some(knowledge::DamageShield::BattleInvolvingRace { race }) => {
                t.ctx.view_data(target).race & race != 0
            }
            None => false,
        }
    });
    if met.defending
        || met.facts.no_damage
        || shielded
        || t.ctx.strike_removes(attacker, target) == Some(true)
        || met.facts.before_damage && met.facts.attacker != crate::knowledge::Fate::Unharmed
    {
        0
    } else {
        (met.facts.attacker_atk.apply(attacker.attack) - met.stat).max(0)
    }
}

fn battle_blocked(t: &Turn, attacker: &CardView, extra_attack: bool) -> bool {
    use crate::staples;
    !t.ctx.attack_locks().is_empty()
        || t.ctx.own_attack_locks().into_iter().any(|lock| {
            match lock.code.map(|code| t.ctx.canonical(code)) {
                Some(staples::GRAVITY_BIND | staples::LEVEL_LIMIT_AREA_B) => {
                    attacker.level >= 4 && !t.ctx.view_data(attacker).is(types::XYZ | types::LINK)
                }
                Some(staples::MESSENGER_OF_PEACE) => attacker.attack >= 1500,
                Some(staples::NIGHTMARES_STEELCAGE) => true,
                // c69408987 turns either player's attacker to Defense after its
                // first battle. Extra attacks do not let it attack in Defense.
                Some(staples::SPIDER_WEB) => extra_attack,
                // Our Swords/Wall/Morphtronic Bind affect only the opponent.
                _ => false,
            }
        })
}

fn own_effect_works(t: &Turn, card: &CardView, source: u32, destroys: bool) -> bool {
    if !t.ctx.reaches(card, source, true, destroys) {
        return false;
    }
    if !card.position.face_up {
        return true;
    }
    // Ctx::reaches deliberately models opposing protection for ordinary
    // removal. A voluntary own-card conversion must also avoid a known
    // indestructible/unaffected friendly card (e.g. an empty-hand Guardian).
    let facts = t.ctx.facts(card);
    let stopped = facts.unaffected | if destroys { facts.immune } else { 0 };
    stopped & crate::knowledge::kind::MONSTER == 0
}

fn best_one_attack(t: &Turn, attacker: &CardView) -> i32 {
    if battle_blocked(t, attacker, false) {
        return 0;
    }
    let enemies = t.ctx.monsters(t.ctx.opp);
    if enemies.is_empty() {
        return attacker.attack.max(0).min(t.ctx.opp_lp().max(0));
    }
    enemies
        .into_iter()
        .filter_map(|target| safe_battle_value(t, attacker, target))
        .max()
        .unwrap_or(0)
}

fn armory_recipient_gain(t: &Turn, target: &CardView) -> Option<i32> {
    if target.at.controller != t.ctx.me
        || target.at.location != Location::MonsterZone
        || !target.position.face_up
        || !target.position.attack
        || !t.ctx.can_attack(target)
        || !own_effect_works(t, target, ARMORY_ARM, false)
    {
        return None;
    }
    let mut boosted = target.clone();
    boosted.attack = boosted.attack.saturating_add(1000);
    if battle_blocked(t, &boosted, false) {
        return None;
    }
    Some(best_one_attack(t, &boosted) - best_one_attack(t, target))
}

fn armory_target_value(t: &Turn, source: &CardView, target: &CardView) -> Option<i32> {
    if target.at == source.at {
        return None;
    }
    let gain = armory_recipient_gain(t, target)?
        - if t.ctx.can_attack(source) {
            best_one_attack(t, source)
        } else {
            0
        };
    (gain > 0).then_some(gain)
}

fn armory(t: &Turn, source: &CardView) -> bool {
    t.ctx.my_turn()
        && t.ctx.main1()
        && t.ctx.obs.turn > 1
        && t.ctx.attack_locks().is_empty()
        && !t.ctx.effects_drained()
        && !t.ctx.wasted(ARMORY_ARM)
        && t.ctx
            .monsters(t.ctx.me)
            .into_iter()
            .any(|target| armory_target_value(t, source, target).is_some())
}

fn trident_cost(t: &Turn, card: &CardView) -> i32 {
    let collateral = if card.position.face_up && t.ctx.is_toon_world(card) {
        t.ctx
            .monsters(t.ctx.me)
            .into_iter()
            .filter(|body| {
                body.position.face_up && t.ctx.view_data(body).in_set(crate::knowledge::SET_TOON)
            })
            .map(|body| {
                keep_value(t, body).max(0)
                    + if t.ctx.can_attack(body) {
                        best_one_attack(t, body)
                    } else {
                        0
                    }
            })
            .sum()
    } else {
        0
    };
    collateral
        + keep_value(t, card).max(0)
        + if t.ctx.can_attack(card) {
            best_one_attack(t, card)
        } else {
            0
        }
}

fn trident(t: &Turn, source: &CardView) -> bool {
    let ctx = t.ctx;
    if !ctx.my_turn()
        || !ctx.main1()
        || ctx.obs.turn <= 1
        || !source.position.attack
        || !ctx.can_attack(source)
        || battle_blocked(t, source, true)
        || ctx.effects_drained()
        || ctx.wasted(TRIDENT_DRAGION)
    {
        return false;
    }
    let enemies = ctx.monsters(ctx.opp);
    if enemies.iter().any(|target| {
        safe_battle_value(t, source, target).is_some()
            && safe_battle_damage(t, source, target) >= ctx.opp_lp()
    }) {
        return false;
    }
    let mut values: Vec<_> = enemies
        .iter()
        .filter_map(|target| safe_battle_value(t, source, target))
        .collect();
    values.sort_unstable_by(|left, right| right.cmp(left));
    // The first attack is already free. Pay for just one further attack;
    // public unknowns or survivors keep blocking any assumed direct attack.
    let extra = if enemies.is_empty() {
        source
            .attack
            .max(0)
            .min((ctx.opp_lp() - source.attack.max(0)).max(0))
    } else if values.len() >= 2 {
        values[1]
    } else if enemies.len() == 1 && values.len() == 1 {
        let first_damage = safe_battle_damage(t, source, enemies[0]);
        source
            .attack
            .max(0)
            .min((ctx.opp_lp() - first_damage).max(0))
    } else {
        0
    };
    extra > 0
        && ctx
            .obs
            .cards
            .iter()
            .filter(|card| {
                card.at.controller == ctx.me
                    && card.at.location.is_field()
                    && card.at != source.at
                    && own_effect_works(t, card, TRIDENT_DRAGION, true)
            })
            .map(|card| trident_cost(t, card))
            .min()
            .is_some_and(|cost| extra > cost)
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
    if monster_source(t, choice, BLACK_ROSE_DRAGON)
        && choice.description == knowledge::description(BLACK_ROSE_DRAGON, 0)
    {
        return Some(global_destruction_profitable(t, BLACK_ROSE_DRAGON, None, 0));
    }
    if monster_source(t, choice, TRIDENT_DRAGION)
        && choice.description == knowledge::description(TRIDENT_DRAGION, 0)
    {
        return Some(trident(t, t.view(choice)?));
    }
    if matches!(t.decision.kind, DecisionKind::Chain { triggers: true, .. }) {
        return None;
    }
    match code {
        JUDGMENT_DRAGON
            if monster_source(t, choice, code)
                && choice.description == knowledge::description(code, 1) =>
        {
            Some(judgment(t, choice))
        }
        ARMORY_ARM
            if monster_source(t, choice, code)
                && choice.description == knowledge::description(code, 0) =>
        {
            Some(armory(t, t.view(choice)?))
        }
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
    if source == ARMORY_ARM && t.decision.hint == Hint::Equip {
        // The actual legal target prompt remains authoritative. Never let a
        // high-ATK enemy outrank a useful friendly equipment destination.
        // The engine excludes the actual handler from its offered targets.
        // Do not guess its zone from a passcode: another Armory copy can be
        // the best legal recipient. Lost source value is constant here.
        return Some(
            armory_recipient_gain(t, card).map_or(-100_000.0, |gain| 10_000.0 + f64::from(gain)),
        );
    }
    if source == TRIDENT_DRAGION
        && t.decision.hint == Hint::Destroy
        && member.at.controller == ctx.me
        && member.at.location.is_field()
    {
        // One extra attack was justified; every further optional destroyed
        // card is still a cost. Negative scores stop at the engine's minimum.
        return Some(if own_effect_works(t, card, TRIDENT_DRAGION, true) {
            -f64::from(trident_cost(t, card).max(1))
        } else {
            -100_000.0
        });
    }
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
                code: JUDGMENT_DRAGON,
                kind: types::MONSTER | types::EFFECT,
                attack: 3000,
                defense: 2600,
                level: 8,
                ..CardData::default()
            },
            CardData {
                code: ARMORY_ARM,
                kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 1800,
                defense: 1200,
                level: 4,
                ..CardData::default()
            },
            CardData {
                code: TRIDENT_DRAGION,
                kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 3000,
                defense: 2800,
                level: 10,
                attribute: attributes::FIRE,
                ..CardData::default()
            },
            CardData {
                code: 26593852,
                kind: types::MONSTER | types::EFFECT | types::SYNCHRO,
                attack: 2200,
                defense: 1200,
                level: 5,
                attribute: attributes::DARK,
                ..CardData::default()
            },
            CardData {
                code: 72302403,
                kind: types::SPELL,
                ..CardData::default()
            },
            CardData {
                code: crate::staples::GRAVITY_BIND,
                kind: types::TRAP | types::CONTINUOUS,
                ..CardData::default()
            },
            CardData {
                code: crate::staples::MESSENGER_OF_PEACE,
                kind: types::SPELL | types::CONTINUOUS,
                ..CardData::default()
            },
            CardData {
                code: crate::staples::NIGHTMARES_STEELCAGE,
                kind: types::SPELL,
                ..CardData::default()
            },
            CardData {
                code: crate::staples::SPIDER_WEB,
                kind: types::SPELL | types::FIELD,
                ..CardData::default()
            },
            CardData {
                code: crate::knowledge::INFERNITY_GUARDIAN,
                kind: types::MONSTER | types::EFFECT,
                attack: 1600,
                defense: 1200,
                level: 4,
                ..CardData::default()
            },
            CardData {
                code: crate::knowledge::THE_FOOL,
                kind: types::MONSTER | types::EFFECT,
                attack: 0,
                defense: 0,
                level: 1,
                ..CardData::default()
            },
            CardData {
                code: 56433456,
                kind: types::SPELL | types::FIELD,
                ..CardData::default()
            },
            CardData {
                code: 53239672,
                kind: types::TRAP | types::CONTINUOUS,
                ..CardData::default()
            },
            CardData {
                code: 77585513,
                kind: types::MONSTER | types::EFFECT,
                attack: 2400,
                level: 6,
                ..CardData::default()
            },
            CardData {
                code: 900_510_007,
                kind: types::MONSTER,
                attack: 1000,
                level: 3,
                race: races::FAIRY,
                attribute: attributes::LIGHT,
                ..CardData::default()
            },
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

    fn transaction_prompt(source: u32, slot: u8, yes: Option<u64>) -> Decision {
        let mut decision = black_rose_prompt(yes);
        decision.subject = yes.map(|_| source);
        if yes.is_none() {
            decision.kind = DecisionKind::Chain {
                forced: false,
                triggers: matches!(source, BLACK_ROSE_DRAGON | TRIDENT_DRAGION),
            };
        }
        for choice in &mut decision.choices {
            choice.card.as_mut().unwrap().code = Some(source);
            choice.description = yes.unwrap_or_else(|| knowledge::description(source, slot));
        }
        decision
    }

    #[test]
    fn judgment_counts_only_other_cards_and_prices_its_lp_cost() {
        let prompt = transaction_prompt(JUDGMENT_DRAGON, 1, None);
        let source = card(JUDGMENT_DRAGON, 0, 0);
        // The native offer can be legal only because it destroys our Bulb-like
        // body. Its existence is not evidence of a beneficial removal target.
        let mut board = obs(vec![source.clone(), card(FODDER, 0, 1)]);
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.cards = vec![source, card(ORDINARY, 1, 0)];
        assert_eq!(black_rose_answer(&board, &prompt), Some(true));
        board.cards.push(card(SMALL, 0, 1));
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.cards.pop();
        board.life_points[0] = 1000;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.life_points[0] = 1500;
        board.cards[1].attack = 900;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
    }

    #[test]
    fn judgment_early_ordering_matches_only_live_exact_ignition() {
        let mut prompt = transaction_prompt(JUDGMENT_DRAGON, 1, None);
        prompt.kind = DecisionKind::Idle;
        let mut board = obs(vec![card(JUDGMENT_DRAGON, 0, 0), card(ORDINARY, 1, 0)]);
        let check = |board: &Observation, prompt: &Decision| {
            let db = db();
            let mut memory = Memory::default();
            let turn = Turn {
                ctx: Ctx::new(board, &db),
                decision: prompt,
                memory: &mut memory,
            };
            activate_before_development(&turn, &prompt.choices[0])
        };
        assert!(check(&board, &prompt));
        prompt.choices[0].description = knowledge::description(JUDGMENT_DRAGON, 2);
        assert!(!check(&board, &prompt));
        assert_eq!(black_rose_answer(&board, &prompt), None);
        prompt.choices[0].description = knowledge::description(JUDGMENT_DRAGON, 1);
        board.phase = Some(Phase::Main2);
        assert!(!check(&board, &prompt));
        board.phase = Some(Phase::Main1);
        prompt.choices[0].card.as_mut().unwrap().at.location = Location::Hand;
        assert!(!check(&board, &prompt));
        assert_eq!(black_rose_answer(&board, &prompt), None);
    }

    #[test]
    fn armory_refuses_enemy_only_targets_and_wasted_open_field_conversion() {
        let prompt = transaction_prompt(ARMORY_ARM, 0, None);
        let mut own_set = card(FODDER, 0, 1);
        own_set.position = Position::FACE_DOWN_DEFENSE;
        let mut board = obs(vec![card(ARMORY_ARM, 0, 0), own_set, card(SMALL, 1, 0)]);
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.cards = vec![card(ARMORY_ARM, 0, 0), card(ORDINARY, 0, 1)];
        assert_eq!(
            black_rose_answer(&board, &prompt),
            Some(false),
            "1000 extra ATK does not replace Armory's own 1800 direct attack"
        );
    }

    #[test]
    fn armory_opens_a_safe_battle_and_selects_the_friendly_recipient() {
        let prompt = transaction_prompt(ARMORY_ARM, 0, None);
        let mut enemy = card(ORDINARY, 1, 0);
        enemy.attack = 2500;
        let mut board = obs(vec![card(ARMORY_ARM, 0, 0), card(ORDINARY, 0, 1), enemy]);
        assert_eq!(black_rose_answer(&board, &prompt), Some(true));
        with_turn(&board, Hint::Equip, Some(ARMORY_ARM), |t| {
            assert!(select(t, &member(&board.cards[1])).unwrap() > 0.0);
            assert!(select(t, &member(&board.cards[2])).unwrap() < 0.0);
        });
        board.cards[1].can_attack = false;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        let mut recovery = transaction_prompt(ARMORY_ARM, 1, None);
        recovery.choices[0].card.as_mut().unwrap().at.location = Location::SpellTrapZone;
        assert_eq!(
            black_rose_answer(&board, &recovery),
            None,
            "SZONE special summon must not inherit the equip guard"
        );
    }

    #[test]
    fn trident_buys_an_incremental_attack_only_when_it_exceeds_own_loss() {
        let prompt = transaction_prompt(TRIDENT_DRAGION, 0, Some(221));
        let mut fodder = card(FODDER, 0, 1);
        fodder.can_attack = false;
        let mut board = obs(vec![card(TRIDENT_DRAGION, 0, 0), fodder]);
        assert_eq!(black_rose_answer(&board, &prompt), Some(true));
        board.life_points[1] = 3000;
        assert_eq!(
            black_rose_answer(&board, &prompt),
            Some(false),
            "the first free attack already wins"
        );
        board.life_points[1] = 8000;
        board.cards[1] = card(ORDINARY, 0, 1);
        assert_eq!(
            black_rose_answer(&board, &prompt),
            Some(false),
            "preserve a valuable live attacker"
        );
        board.cards[1] = card(FODDER, 0, 1);
        board.cards.push(card(SMALL, 1, 0));
        assert_eq!(black_rose_answer(&board, &prompt), Some(true));
        board.life_points[1] = 1800;
        assert_eq!(
            black_rose_answer(&board, &prompt),
            Some(false),
            "a free battle already deals lethal damage"
        );
    }

    #[test]
    fn trident_rejects_fatal_or_unknown_blockers_and_unavailable_battle() {
        let prompt = transaction_prompt(TRIDENT_DRAGION, 0, Some(221));
        let mut board = obs(vec![
            card(TRIDENT_DRAGION, 0, 0),
            card(FODDER, 0, 1),
            card(26593852, 1, 0),
        ]);
        assert_eq!(
            black_rose_answer(&board, &prompt),
            Some(false),
            "Catastor destroys FIRE Trident before damage"
        );
        board.cards[2] = card(SMALL, 1, 0);
        board.cards[2].code = None;
        board.cards[2].position = Position::FACE_DOWN_DEFENSE;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.cards.pop();
        for phase in [Phase::Main2, Phase::End] {
            board.phase = Some(phase);
            assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        }
        board.phase = Some(Phase::Main1);
        board.cards[0].can_attack = false;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.cards[0].can_attack = true;
        board.turn = 1;
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
        board.turn = 2;
        let mut lock = card(72302403, 1, 0);
        lock.at.location = Location::SpellTrapZone;
        board.cards.push(lock);
        assert_eq!(black_rose_answer(&board, &prompt), Some(false));
    }

    #[test]
    fn trident_trigger_identity_and_actual_cost_ranking_remain_narrow() {
        let board = obs(vec![
            card(TRIDENT_DRAGION, 0, 0),
            card(FODDER, 0, 1),
            card(ORDINARY, 0, 2),
        ]);
        for description in [0, 221, knowledge::description(TRIDENT_DRAGION, 0)] {
            assert_eq!(
                black_rose_answer(
                    &board,
                    &transaction_prompt(TRIDENT_DRAGION, 0, Some(description))
                ),
                Some(true)
            );
        }
        assert_eq!(
            black_rose_answer(&board, &transaction_prompt(TRIDENT_DRAGION, 0, None)),
            Some(true)
        );
        for description in [31, 94, knowledge::description(TRIDENT_DRAGION, 1)] {
            assert_eq!(
                black_rose_answer(
                    &board,
                    &transaction_prompt(TRIDENT_DRAGION, 0, Some(description))
                ),
                None
            );
        }
        with_turn(&board, Hint::Destroy, Some(TRIDENT_DRAGION), |t| {
            let cheap = select(t, &member(&board.cards[1])).unwrap();
            let dear = select(t, &member(&board.cards[2])).unwrap();
            assert!(0.0 > cheap && cheap > dear);
        });
    }

    #[test]
    fn generic_honors_native_transaction_declines() {
        use crate::agent::{Agent, Policy};
        use std::sync::Arc;
        for (source, slot, yes, board) in [
            (
                JUDGMENT_DRAGON,
                1,
                None,
                obs(vec![card(JUDGMENT_DRAGON, 0, 0), card(FODDER, 0, 1)]),
            ),
            (
                ARMORY_ARM,
                0,
                None,
                obs(vec![card(ARMORY_ARM, 0, 0), card(SMALL, 1, 0)]),
            ),
            (
                TRIDENT_DRAGION,
                0,
                Some(221),
                obs(vec![
                    card(TRIDENT_DRAGION, 0, 0),
                    card(FODDER, 0, 1),
                    card(26593852, 1, 0),
                ]),
            ),
        ] {
            let mut prompt = transaction_prompt(source, slot, yes);
            if yes.is_none() {
                prompt.kind = DecisionKind::Chain {
                    forced: false,
                    triggers: false,
                };
            }
            let mut pilot = Agent::seeded(Generic, Arc::new(db()), 1);
            assert_eq!(pilot.choose(&board, &prompt), 1, "source {source}");
        }
    }

    #[test]
    fn generic_keeps_friendly_equip_intent_and_stops_after_one_trident_cost() {
        use crate::agent::{Agent, Policy};
        use std::sync::Arc;
        let mut enemy = card(ORDINARY, 1, 0);
        enemy.attack = 2500;
        let board = obs(vec![card(ARMORY_ARM, 0, 0), card(ORDINARY, 0, 1), enemy]);
        let mut pilot = Agent::seeded(Generic, Arc::new(db()), 1);
        assert_eq!(
            pilot.choose(&board, &transaction_prompt(ARMORY_ARM, 0, None)),
            0
        );
        let mut select_prompt = transaction_prompt(ARMORY_ARM, 0, None);
        select_prompt.kind = DecisionKind::SelectCards;
        select_prompt.hint = Hint::Equip;
        select_prompt.minimum = 1;
        select_prompt.maximum = 1;
        select_prompt.choices = [2, 1]
            .into_iter()
            .map(|index| Choice {
                kind: ChoiceKind::Cards,
                card: None,
                members: vec![member(&board.cards[index])],
                description: 0,
                place: None,
            })
            .collect();
        assert_eq!(pilot.choose(&board, &select_prompt), 1);

        let board = obs(vec![
            card(TRIDENT_DRAGION, 0, 0),
            card(FODDER, 0, 1),
            card(ORDINARY, 0, 2),
        ]);
        let mut pilot = Agent::seeded(Generic, Arc::new(db()), 1);
        assert_eq!(
            pilot.choose(&board, &transaction_prompt(TRIDENT_DRAGION, 0, Some(221))),
            0
        );
        select_prompt.hint = Hint::Destroy;
        select_prompt.maximum = 2;
        select_prompt.choices = [vec![1], vec![1, 2]]
            .into_iter()
            .map(|indices| Choice {
                kind: ChoiceKind::Cards,
                card: None,
                members: indices
                    .into_iter()
                    .map(|index| member(&board.cards[index]))
                    .collect(),
                description: 0,
                place: None,
            })
            .collect();
        assert_eq!(pilot.choose(&board, &select_prompt), 0);
        select_prompt.kind = DecisionKind::SelectToggle;
        select_prompt.selected = vec![board.cards[1].at];
        select_prompt.choices = vec![
            Choice {
                kind: ChoiceKind::Toggle,
                card: Some(member(&board.cards[2])),
                members: vec![],
                description: 0,
                place: None,
            },
            Choice {
                kind: ChoiceKind::Finish,
                card: None,
                members: vec![],
                description: 0,
                place: None,
            },
        ];
        assert_eq!(pilot.choose(&board, &select_prompt), 1);
    }

    #[test]
    fn own_two_sided_locks_block_conversion_but_own_swords_does_not() {
        for lock_code in [
            crate::staples::GRAVITY_BIND,
            crate::staples::MESSENGER_OF_PEACE,
            crate::staples::NIGHTMARES_STEELCAGE,
            crate::staples::SPIDER_WEB,
            72302403,
        ] {
            let mut lock = card(lock_code, 0, 0);
            lock.at.location = Location::SpellTrapZone;
            lock.at.sequence = if lock_code == crate::staples::SPIDER_WEB {
                5
            } else {
                0
            };
            lock.can_attack = false;
            let board = obs(vec![
                card(TRIDENT_DRAGION, 0, 0),
                card(FODDER, 0, 1),
                lock.clone(),
            ]);
            assert_eq!(
                black_rose_answer(&board, &transaction_prompt(TRIDENT_DRAGION, 0, Some(221))),
                Some(lock_code == 72302403),
                "lock {lock_code}"
            );
            if lock_code != crate::staples::SPIDER_WEB {
                let mut enemy = card(ORDINARY, 1, 0);
                enemy.attack = 2500;
                let board = obs(vec![
                    card(ARMORY_ARM, 0, 0),
                    card(ORDINARY, 0, 1),
                    enemy,
                    lock,
                ]);
                assert_eq!(
                    black_rose_answer(&board, &transaction_prompt(ARMORY_ARM, 0, None)),
                    Some(lock_code == 72302403),
                    "lock {lock_code}"
                );
            }
        }
    }

    #[test]
    fn trident_uses_only_working_costs_in_activation_and_actual_selection() {
        use crate::model::{Coin, CoinEffect};
        for code in [
            crate::knowledge::INFERNITY_GUARDIAN,
            crate::knowledge::THE_FOOL,
        ] {
            let mut protected = card(code, 0, 1);
            protected.can_attack = false;
            if code == crate::knowledge::THE_FOOL {
                protected.coin_effect = Some(CoinEffect {
                    code,
                    result: Coin::Heads,
                });
            }
            let mut board = obs(vec![card(TRIDENT_DRAGION, 0, 0), protected]);
            let prompt = transaction_prompt(TRIDENT_DRAGION, 0, Some(221));
            assert_eq!(black_rose_answer(&board, &prompt), Some(false));
            let mut working = card(ORDINARY, 0, 2);
            working.can_attack = false;
            board.cards.push(working);
            assert_eq!(black_rose_answer(&board, &prompt), Some(true));
            with_turn(&board, Hint::Destroy, Some(TRIDENT_DRAGION), |t| {
                assert!(
                    select(t, &member(&board.cards[2])).unwrap()
                        > select(t, &member(&board.cards[1])).unwrap()
                );
            });
        }
        let mut world = card(crate::knowledge::TOON_WORLD, 0, 0);
        world.at.location = Location::SpellTrapZone;
        world.can_attack = false;
        let board = obs(vec![
            card(TRIDENT_DRAGION, 0, 0),
            card(TOON_BODY, 0, 1),
            world,
        ]);
        assert_eq!(
            black_rose_answer(&board, &transaction_prompt(TRIDENT_DRAGION, 0, Some(221))),
            Some(false),
            "a cheap-looking World would also destroy its valuable Toon"
        );
    }

    #[test]
    fn second_armory_copy_can_equip_the_first_without_guessing_handler_identity() {
        use crate::agent::{Agent, Policy};
        use std::sync::Arc;
        let mut enemy = card(ORDINARY, 1, 0);
        enemy.attack = 2500;
        let board = obs(vec![card(ARMORY_ARM, 0, 0), card(ARMORY_ARM, 0, 1), enemy]);
        let mut prompt = transaction_prompt(ARMORY_ARM, 0, None);
        prompt.choices[0].card.as_mut().unwrap().at.sequence = 1;
        let mut pilot = Agent::seeded(Generic, Arc::new(db()), 1);
        assert_eq!(pilot.choose(&board, &prompt), 0);
        prompt.kind = DecisionKind::SelectCards;
        prompt.hint = Hint::Equip;
        prompt.minimum = 1;
        prompt.maximum = 1;
        // The real source is absent from this native-shaped legal target list.
        prompt.choices = [2, 0]
            .into_iter()
            .map(|index| Choice {
                kind: ChoiceKind::Cards,
                card: None,
                members: vec![member(&board.cards[index])],
                description: 0,
                place: None,
            })
            .collect();
        assert_eq!(pilot.choose(&board, &prompt), 1);
    }

    #[test]
    fn extra_attack_damage_respects_pre_damage_removal_and_public_shields() {
        let mut enemy = card(900_510_007, 1, 0);
        enemy.attack = 2500;
        let board = obs(vec![card(ARMORY_ARM, 0, 0), card(26593852, 0, 1), enemy]);
        assert_eq!(
            black_rose_answer(&board, &transaction_prompt(ARMORY_ARM, 0, None)),
            Some(false),
            "boosting Catastor cannot add damage when it removes the target before calculation"
        );
        for shield_code in [56433456, 53239672] {
            let mut shield = card(shield_code, 1, 0);
            shield.at.location = Location::SpellTrapZone;
            shield.at.sequence = if shield_code == 56433456 { 5 } else { 0 };
            shield.can_attack = false;
            let mut board = obs(vec![
                card(TRIDENT_DRAGION, 0, 0),
                card(FODDER, 0, 1),
                card(900_510_007, 1, 0),
                shield,
            ]);
            board.life_points[1] = 2000;
            let prompt = transaction_prompt(TRIDENT_DRAGION, 0, Some(221));
            assert_eq!(
                black_rose_answer(&board, &prompt),
                Some(true),
                "first battle deals zero, second direct attack is useful: shield {shield_code}"
            );
            if shield_code == 53239672 {
                board.cards.push(card(77585513, 0, 2));
                assert_eq!(
                    black_rose_answer(&board, &prompt),
                    Some(false),
                    "Jinzo suppresses Spirit Barrier, so first attack already wins"
                );
                board.cards.last_mut().unwrap().at.location = Location::SpellTrapZone;
                assert_eq!(
                    black_rose_answer(&board, &prompt),
                    Some(true),
                    "equipped Jinzo does not suppress field traps"
                );
            }
        }
    }

    #[test]
    fn own_level_locks_do_not_treat_an_unfamiliar_rank_as_a_level() {
        let mut lock = card(crate::staples::GRAVITY_BIND, 0, 0);
        lock.at.location = Location::SpellTrapZone;
        let body = card(ORDINARY, 0, 0);
        let board = obs(vec![body.clone(), lock]);
        let mut database = db();
        let mut memory = Memory::default();
        let prompt = transaction_prompt(ARMORY_ARM, 0, None);
        for kind in [types::XYZ, types::LINK] {
            database.0.get_mut(&ORDINARY).unwrap().kind = types::MONSTER | kind;
            let t = Turn {
                ctx: Ctx::new(&board, &database),
                decision: &prompt,
                memory: &mut memory,
            };
            assert!(!battle_blocked(&t, &body, true));
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
